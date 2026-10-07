use super::*;
use eframe::{egui_wgpu, wgpu};
use icy_engine_gui::TerminalShaderRenderer;

#[derive(rust_embed::RustEmbed)]
#[folder = "../icy_engine_gui/i18n"]
struct GuiLocalizations;

/// Tests assert on English labels, so pin both loaders to English regardless of the system locale.
pub(super) fn use_english() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let english: [i18n_embed::unic_langid::LanguageIdentifier; 1] = ["en".parse().unwrap()];
        icy_draw::select_languages(&english);
        i18n_embed::select(&*icy_engine_gui::LANGUAGE_LOADER, &GuiLocalizations, &english).unwrap();
    });
}

pub(super) fn frame(context: &egui::Context, app: &mut DrawApp, size: egui::Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
    let time = context.input(|input| input.time) + 0.05;
    let modifiers = events
        .iter()
        .find_map(|event| match event {
            egui::Event::Key { modifiers, .. } | egui::Event::PointerButton { modifiers, .. } | egui::Event::MouseWheel { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    context.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            modifiers,
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
fn command_wheel_zooms_the_canvas() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![]);
    let pointer = app.canvas_rect.center();
    let before = app.view.zoom;
    frame(
        &context,
        &mut app,
        size,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 100.0),
                modifiers: egui::Modifiers::COMMAND,
            },
        ],
    );
    assert!(matches!(app.settings.monitor_settings.scaling_mode, ScalingMode::Manual(zoom) if zoom > before));
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
fn font_import_menu_picker_preview_and_import_open_a_new_font() {
    use_english();
    for bitmap in [false, true] {
        let context = egui::Context::default();
        appearance::apply(&context);
        let mut app = DrawApp::new();
        if bitmap {
            app.create(NewKind::BitmapFont, Size::new(80, 25));
        }
        let size = egui::vec2(1000.0, 760.0);
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        click_text(&context, &mut app, size, "File");
        click_text(&context, &mut app, size, &fl!("menu-import-font"));
        assert!(matches!(app.dialog, Some(Dialog::FontImport(_))));
        frame(&context, &mut app, size, vec![key_event(Key::Enter, egui::Modifiers::NONE)]);
        assert!(matches!(app.dialog, Some(Dialog::FontImport(_))), "import without a preview is disabled");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("import.psf");
        let font = icy_engine::BitFont::from_sauce_name("IBM VGA 850").unwrap();
        let bytes = font.to_psf2_bytes().unwrap();
        std::fs::write(&path, &bytes).unwrap();
        app.picker = true;
        app.sender
            .send(Picked {
                action: FileAction::ImportFont,
                path: Some(path.clone()),
            })
            .unwrap();
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        assert!(!app.picker);
        let output = frame(&context, &mut app, size, vec![]);
        assert!(text_position(&output, "import").is_some());
        click_text(&context, &mut app, size, &fl!("font-import-button"));
        assert!(app.dialog.is_none());
        let editor = app.font_editor.as_ref().unwrap();
        assert_eq!(editor.state.build_font().convert_to_u8_data(), font.convert_to_u8_data());
        assert!(editor.path.is_none());
        assert!(!editor.apply_target);
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "source is never overwritten");
    }
}

#[test]
fn font_export_menu_and_shortcut_preserve_the_font_save_state() {
    use_english();
    for shortcut in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.psf");
        let target = directory.path().join("sheet.png");
        let context = egui::Context::default();
        appearance::apply(&context);
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        let editor = app.font_editor.as_mut().unwrap();
        editor.save(&source, false).unwrap();
        let source_bytes = std::fs::read(&source).unwrap();
        let before = editor.state.get_glyph_pixels('A')[0][0];
        editor.state.set_pixel('A', 0, 0, !before).unwrap();
        let expected = editor.state.build_font();
        let size = egui::vec2(1000.0, 760.0);
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        if shortcut {
            frame(
                &context,
                &mut app,
                size,
                vec![key_event(Key::E, egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT))],
            );
        } else {
            click_text(&context, &mut app, size, "File");
            click_text(&context, &mut app, size, &fl!("menu-export-font"));
        }
        assert!(matches!(app.dialog, Some(Dialog::FontExport(_))));
        app.picker = true;
        app.sender
            .send(Picked {
                action: FileAction::ExportFont {
                    file_name: "sheet.png".into(),
                    extension: "png".into(),
                },
                path: Some(target.clone()),
            })
            .unwrap();
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        click_text(&context, &mut app, size, &fl!("font-export-button"));
        assert!(app.dialog.is_none());
        let exported = image::open(&target).unwrap().to_luma8();
        assert_eq!(exported, icy_draw::font_export::image_export::font_image(&expected));
        let editor = app.font_editor.as_ref().unwrap();
        assert_eq!(editor.path.as_deref(), Some(source.as_path()));
        assert!(editor.modified(), "export must not mark edits as saved");
        assert_eq!(editor.state.build_font().convert_to_u8_data(), expected.convert_to_u8_data());
        assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    }
}

#[test]
fn font_export_format_selection_and_overwrite_confirmation_are_wired() {
    use_english();
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("export.psf");
    std::fs::write(&target, b"KEEP").unwrap();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.create(NewKind::BitmapFont, Size::new(80, 25));
    let expected = app.font_editor.as_ref().unwrap().state.build_font().convert_to_u8_data();
    app.open_font_export();
    let size = egui::vec2(1000.0, 760.0);
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    click_text(&context, &mut app, size, "PNG Image");
    click_text(&context, &mut app, size, "PSF (Linux Console)");
    if let Some(Dialog::FontExport(draft)) = &mut app.dialog {
        assert_eq!(draft.extension(), "psf");
        draft.set_path(&target);
    }
    click_text(&context, &mut app, size, &fl!("font-export-button"));
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    assert_eq!(std::fs::read(&target).unwrap(), b"KEEP");
    click_text(&context, &mut app, size, &labels::cancel());
    assert!(matches!(app.dialog, Some(Dialog::FontExport(_))));
    assert_eq!(std::fs::read(&target).unwrap(), b"KEEP");
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    click_text(&context, &mut app, size, &fl!("font-export-button"));
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    click_text(&context, &mut app, size, &labels::overwrite());
    assert!(app.dialog.is_none());
    let loaded = icy_engine::BitFont::from_bytes("Export", &std::fs::read(&target).unwrap()).unwrap();
    assert_eq!(loaded.convert_to_u8_data(), expected);
    assert!(app.font_editor.as_ref().unwrap().path.is_none());
}

#[test]
fn font_export_com_subformats_are_selectable_and_cancel_preserves_edits() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.create(NewKind::BitmapFont, Size::new(80, 25));
    let editor = app.font_editor.as_mut().unwrap();
    editor.apply_target = true;
    let before = editor.state.get_glyph_pixels('A')[0][0];
    editor.state.set_pixel('A', 0, 0, !before).unwrap();
    app.open_font_export();
    let size = egui::vec2(1000.0, 760.0);
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    click_text(&context, &mut app, size, "PNG Image");
    click_text(&context, &mut app, size, "DOS COM Executable");
    click_text(&context, &mut app, size, "Non-TSR (simple)");
    click_text(&context, &mut app, size, "TSR (all modes)");
    let output = frame(&context, &mut app, size, vec![]);
    assert!(text_position(&output, "TSR (all modes)").is_some());
    app.picker = true;
    frame(&context, &mut app, size, vec![key_event(Key::Escape, egui::Modifiers::NONE)]);
    assert!(matches!(app.dialog, Some(Dialog::FontExport(_))));
    app.sender
        .send(Picked {
            action: FileAction::ExportFont {
                file_name: "export.com".into(),
                extension: "com".into(),
            },
            path: None,
        })
        .unwrap();
    frame(&context, &mut app, size, vec![]);
    assert!(!app.picker);
    click_text(&context, &mut app, size, &labels::cancel());
    assert!(app.dialog.is_none());
    let editor = app.font_editor.as_ref().unwrap();
    assert!(editor.apply_target);
    assert!(editor.modified());
    assert_eq!(editor.state.get_glyph_pixels('A')[0][0], !before);
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_font_export_dialog_renders_desktop_and_compact_layouts() {
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        for format in [icy_draw::font_export::FontExportFormat::Png, icy_draw::font_export::FontExportFormat::Com] {
            for size in [[1280, 820], [440, 700]] {
                app.open_font_export();
                if let Some(Dialog::FontExport(draft)) = &mut app.dialog {
                    draft.set_format(format);
                }
                for _ in 0..3 {
                    gpu.capture(&mut app, size, 1.0, vec![], "font-export-warmup");
                }
                let pixels = gpu.capture(&mut app, size, 1.0, vec![], &format!("font-export-{format:?}-{}", size[0]));
                assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] > 180 && pixel[1] > 180 && pixel[2] > 180));
            }
        }
    });
}

#[test]
fn font_import_cancel_and_discard_preserve_unsaved_edits_until_confirmed() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1000.0, 760.0);
    let mut app = DrawApp::new();
    app.document.type_text("KEEP").unwrap();
    let original = app.document.screen.clone();
    app.import_font(icy_engine::BitFont::default());
    assert!(matches!(app.dialog, Some(Dialog::Close)));
    assert!(std::sync::Arc::ptr_eq(&original, &app.document.screen));
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    click_text(&context, &mut app, size, &labels::cancel());
    assert!(app.pending_font.is_none());
    assert!(std::sync::Arc::ptr_eq(&original, &app.document.screen));
    assert!(app.font_editor.is_none());
    app.import_font(icy_engine::BitFont::default());
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    click_text(&context, &mut app, size, &fl!("ask_close_file_dialog-dont_save_button"));
    assert!(app.font_editor.is_some());
    assert!(app.pending_font.is_none());
    assert!(!std::sync::Arc::ptr_eq(&original, &app.document.screen));
}

#[test]
fn font_import_save_completes_pending_import_and_protects_font_edits() {
    let context = egui::Context::default();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("keep.icy");
    let mut app = DrawApp::new();
    app.document.type_text("KEEP").unwrap();
    app.import_font(icy_engine::BitFont::default());
    app.continue_after_save = true;
    app.save_path(&context, path.clone(), false);
    assert!(app.pending_font.is_none());
    assert!(app.font_editor.is_some());
    let saved = Document::load(&path).unwrap();
    assert_eq!(saved.with_state(|state| state.get_buffer().char_at((0, 0).into()).ch), 'K');

    let editor = app.font_editor.as_mut().unwrap();
    editor.apply_target = true;
    let before = editor.state.get_glyph_pixels('A')[0][0];
    editor.state.set_pixel('A', 0, 0, !before).unwrap();
    app.import_font(icy_engine::BitFont::default());
    assert!(matches!(app.dialog, Some(Dialog::Close)));
    assert_eq!(app.font_editor.as_ref().unwrap().state.get_glyph_pixels('A')[0][0], !before);
}

#[test]
fn font_import_failed_or_cancelled_save_clears_pending_import() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.document.type_text("KEEP").unwrap();
    app.import_font(icy_engine::BitFont::default());
    app.continue_after_save = true;
    app.save_path(&context, PathBuf::from("/nonexistent/icy-draw/keep.icy"), false);
    assert!(matches!(app.dialog, Some(Dialog::Error(_))));
    assert!(app.pending_font.is_none());
    assert!(app.document.modified());
    assert!(app.font_editor.is_none());

    app.import_font(icy_engine::BitFont::default());
    app.continue_after_save = true;
    app.dialog = None;
    app.picker = true;
    app.sender
        .send(Picked {
            action: FileAction::Save,
            path: None,
        })
        .unwrap();
    frame(&context, &mut app, egui::vec2(1000.0, 760.0), vec![]);
    assert!(app.pending_font.is_none());
    assert!(app.document.modified());
    assert!(app.font_editor.is_none());
}

#[test]
fn font_import_cancelled_picker_and_escape_keep_the_document() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let original = app.document.screen.clone();
    app.dialog = Some(Dialog::FontImport(Box::default()));
    app.picker = true;
    app.sender
        .send(Picked {
            action: FileAction::ImportFont,
            path: None,
        })
        .unwrap();
    let size = egui::vec2(1000.0, 760.0);
    frame(&context, &mut app, size, vec![]);
    assert!(!app.picker);
    assert!(matches!(app.dialog, Some(Dialog::FontImport(_))));
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    frame(&context, &mut app, size, vec![key_event(Key::Escape, egui::Modifiers::NONE)]);
    assert!(app.dialog.is_none());
    assert!(std::sync::Arc::ptr_eq(&original, &app.document.screen));
}

#[test]
fn font_import_xbin_selection_imports_the_selected_font() {
    use_english();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("two.xb");
    let mut data = b"XBIN\x1A\x01\x00\x01\x00\x10\x12".to_vec();
    data.extend(vec![0x80; 256 * 16]);
    data.extend(vec![0x40; 256 * 16]);
    data.extend([0, 7]);
    std::fs::write(&path, data).unwrap();
    for (label, expected) in [(fl!("font-import-xb-font-1"), 0x80), (fl!("font-import-xb-font-2"), 0x40)] {
        let context = egui::Context::default();
        appearance::apply(&context);
        let mut app = DrawApp::new();
        let mut draft = font_import::FontImportDialog::default();
        draft.load(&path);
        app.dialog = Some(Dialog::FontImport(Box::new(draft)));
        let size = egui::vec2(1000.0, 760.0);
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        click_text(&context, &mut app, size, &label);
        click_text(&context, &mut app, size, &fl!("font-import-button"));
        assert!(app.dialog.is_none());
        let data = app.font_editor.as_ref().unwrap().state.build_font().convert_to_u8_data();
        assert_eq!(data.len(), 256 * 16);
        assert!(data.iter().all(|byte| *byte == expected), "{label} must keep XBin slot order");
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_font_import_dialog_renders_empty_and_loaded_previews() {
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.psf");
        std::fs::write(&path, icy_engine::BitFont::default().to_psf2_bytes().unwrap()).unwrap();
        for size in [[1280, 820], [440, 700]] {
            app.dialog = Some(Dialog::FontImport(Box::default()));
            for _ in 0..3 {
                gpu.capture(&mut app, size, 1.0, vec![], "font-import-warmup");
            }
            let empty = gpu.capture(&mut app, size, 1.0, vec![], &format!("font-import-empty-{}", size[0]));
            if let Some(Dialog::FontImport(draft)) = &mut app.dialog {
                draft.load(&path);
            }
            for _ in 0..3 {
                gpu.capture(&mut app, size, 1.0, vec![], "font-import-warmup");
            }
            let loaded = gpu.capture(&mut app, size, 1.0, vec![], &format!("font-import-preview-{}", size[0]));
            let changed = empty
                .chunks_exact(4)
                .zip(loaded.chunks_exact(4))
                .filter(|(before, after)| before != after)
                .count();
            assert!(changed > 500, "the loaded glyph sheet must be rendered at {size:?}");
        }
    });
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
fn clipboard_images_and_icy_data_paste_into_floating_layers() {
    let mut app = DrawApp::new();
    app.document.with_state(|state| state.set_caret_foreground(12));
    app.document.type_text("AB").unwrap();
    let mut selection = Selection::new(Position::new(0, 0));
    selection.lead = Position::new(1, 0);
    app.document.with_state(|state| state.set_selection(selection)).unwrap();
    let data = icy_engine_gui::prepare_clipboard_data(&**app.document.screen.lock()).unwrap();
    assert!(data.rtf.is_some() && data.image.is_some(), "a selection is copied as rich text and image too");
    app.document.with_state(|state| state.set_caret_position(Position::new(0, 3)));
    app.paste_content(icy_engine_gui::system_clipboard::PasteContent::Icy {
        text: data.text.clone(),
        data: data.icy_data.unwrap(),
    });
    assert!(app.document.paste_active());
    assert_eq!(
        app.document
            .with_state(|state| state.get_buffer().char_at(Position::new(0, 3)).attribute.foreground()),
        12
    );
    let _ = app.document.paste_action(icy_draw::document::PasteAction::Keep);

    let layers = app.document.with_state(|state| state.get_buffer().layers.len());
    app.paste_content(icy_engine_gui::system_clipboard::PasteContent::Image(image::RgbaImage::from_pixel(
        16,
        16,
        image::Rgba([255, 0, 0, 255]),
    )));
    assert!(app.document.paste_active(), "an image pastes as a floating layer");
    assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), layers + 1);
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
fn unrestricted_font_window_adds_selects_and_replaces_slots_with_undo() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.document
        .with_state(|state| state.get_buffer_mut().font_mode = icy_engine::FontMode::Unlimited);
    app.open_font_selector();
    assert!(app.font_slots_open);
    assert!(app.dialog.is_none());
    let (slots, active) = app.font_slot_entries();
    assert_eq!(active, 0);
    assert_eq!(slots.len(), icy_engine::ANSI_SLOT_COUNT);
    assert_eq!(
        slots[42].1,
        app.document.with_state(|state| state.get_buffer().font(42).unwrap().name().to_owned())
    );
    frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    let output = frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    assert!(labels.iter().any(|text| text.contains("Document Fonts")), "{labels:?}");
    assert!(labels.iter().any(|text| text.contains("Predefined fonts")), "{labels:?}");

    app.document.with_state(|state| state.switch_to_font_page(42)).unwrap();
    assert_eq!(app.document.with_state(|state| state.get_caret().font_page()), 42);
    assert!(app.document.with_state(|state| state.get_buffer().has_font(42)));
    app.document.undo().unwrap();
    assert_eq!(app.document.with_state(|state| state.get_caret().font_page()), 0);
    assert!(!app.document.with_state(|state| state.get_buffer().has_font(42)));

    let mut first = icy_engine::BitFont::default();
    first.set_name("First");
    app.choose_font(FontSelectionTarget::Add);
    app.apply_font(first.clone());
    let first_page = app.document.with_state(|state| state.get_caret().font_page());
    let mut second = icy_engine::BitFont::default();
    second.set_name("Second");
    app.choose_font(FontSelectionTarget::Add);
    app.apply_font(second.clone());
    let second_page = app.document.with_state(|state| state.get_caret().font_page());
    assert_ne!(first_page, second_page);
    app.document.with_state(|state| {
        assert_eq!(state.get_buffer().font(first_page), Some(&first));
        assert_eq!(state.get_buffer().font(second_page), Some(&second));
    });
    app.document.undo().unwrap();
    assert_eq!(app.document.with_state(|state| state.get_caret().font_page()), first_page);
    app.document.redo().unwrap();
    assert_eq!(app.document.with_state(|state| state.get_caret().font_page()), second_page);

    let mut replacement = icy_engine::BitFont::default();
    replacement.set_name("Replacement");
    app.choose_font(FontSelectionTarget::Replace(first_page));
    app.apply_font(replacement.clone());
    assert_eq!(app.document.with_state(|state| state.get_buffer().font(first_page).cloned()), Some(replacement));
    app.document.undo().unwrap();
    assert_eq!(app.document.with_state(|state| state.get_buffer().font(first_page).cloned()), Some(first));
}

#[test]
fn sauce_font_status_keeps_the_existing_font_picker() {
    let mut app = DrawApp::new();
    app.open_font_selector();
    assert!(!app.font_slots_open);
    assert!(matches!(app.dialog, Some(Dialog::FontSelect)));
}

#[test]
fn internal_copy_paste_starts_opaque_and_transparency_toggles() {
    let mut app = DrawApp::new();
    app.document.type_text("A B").unwrap();
    app.document.with_state(|state| state.set_caret_position(Position::new(0, 2)));
    app.document.type_text("XXX").unwrap();
    let mut selection = Selection::new(Position::new(0, 0));
    selection.lead = Position::new(2, 0);
    app.document.with_state(|state| state.set_selection(selection)).unwrap();
    app.copy(&egui::Context::default());
    let text = app.clipboard.as_ref().unwrap().0.clone();
    app.document.with_state(|state| state.set_caret_position(Position::new(0, 2)));
    app.paste(&text);

    let pasted = |app: &DrawApp| app.document.with_state(|state| state.get_buffer().char_at(Position::new(1, 2)));
    assert!(app.document.paste_active());
    assert!(!app.document.paste_transparent());
    assert_eq!(pasted(&app).ch, ' ', "copied spaces must cover the content beneath");
    app.document.paste_action(icy_draw::document::PasteAction::Transparent).unwrap();
    assert_eq!(pasted(&app).ch, 'X');
    app.document.paste_action(icy_draw::document::PasteAction::Transparent).unwrap();
    assert_eq!(pasted(&app).ch, ' ');
}

#[test]
fn copying_unpainted_cells_from_opaque_layer_covers_destination() {
    for alpha in [false, true] {
        let mut app = DrawApp::new();
        app.document
            .with_state(|state| state.get_cur_layer_mut().unwrap().properties.has_alpha_channel = alpha);
        app.document.type_text("A").unwrap();
        app.document.with_state(|state| state.set_caret_position(Position::new(2, 0)));
        app.document.type_text("B").unwrap();
        app.document.with_state(|state| state.set_caret_position(Position::new(0, 2)));
        app.document.type_text("XXX").unwrap();
        let mut selection = Selection::new(Position::new(0, 0));
        selection.lead = Position::new(2, 0);
        app.document.with_state(|state| state.set_selection(selection)).unwrap();
        app.copy(&egui::Context::default());
        let text = app.clipboard.as_ref().unwrap().0.clone();
        app.document.with_state(|state| state.set_caret_position(Position::new(0, 2)));
        app.paste(&text);

        let at = |app: &DrawApp| app.document.with_state(|state| state.get_buffer().char_at(Position::new(1, 2)));
        assert_eq!(at(&app).ch, if alpha { 'X' } else { ' ' }, "alpha={alpha}");
        if !alpha {
            app.document.paste_action(icy_draw::document::PasteAction::Transparent).unwrap();
            assert_eq!(at(&app).ch, 'X');
            app.document.paste_action(icy_draw::document::PasteAction::Transparent).unwrap();
            assert_eq!(at(&app).ch, ' ');
        }
    }
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
    app.settings.last_export_directory = None;
    app.settings.export_settings = Default::default();
    let request = app.export_dialog().with_format(FileFormat::Ansi).request().unwrap();
    let output = directory.path().join("art.ans");
    assert_eq!(request.path, output, "exports go next to the document by default");
    app.export(&request).unwrap();
    assert!(app.document.modified());
    assert_eq!(app.document.path, Some(native));
    let reopened = Document::load(&output).unwrap();
    assert_eq!(reopened.with_state(|state| state.get_sauce_meta().author.to_string()), "Artist");
    assert_eq!(reopened.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'A');
    let image = app
        .export_dialog()
        .with_format(FileFormat::Image(icy_engine::formats::ImageFormat::Png))
        .request()
        .unwrap();
    app.export(&image).unwrap();
    assert!(image::open(&image.path).unwrap().width() > 0);
    let native = app.export_dialog().with_format(FileFormat::IcyDraw).request().unwrap();
    assert!(app.export(&native).is_err(), "exporting must not replace the document itself");
}

#[test]
fn atascii_documents_keep_their_screen_through_saving_and_exporting() {
    use icy_draw::screen_profile::{AtasciiMode, ScreenProfile};
    use_english();
    let directory = tempfile::tempdir().unwrap();
    for mode in AtasciiMode::ALL {
        let mut app = DrawApp::new();
        app.new_atascii_mode = mode;
        app.create(NewKind::Atascii, Size::new(80, 25));
        assert_eq!(app.document.profile(), ScreenProfile::Atascii(mode));
        let font = |document: &Document| document.with_state(|state| state.get_buffer().font(0).unwrap().name().to_string());
        app.document.type_text("HI").unwrap();

        let native = directory.path().join(format!("{}.icy", mode.extension()));
        app.document.save(&native, false).unwrap();
        let reopened = Document::load(&native).unwrap();
        assert_eq!(reopened.profile(), ScreenProfile::Atascii(mode), "the .icy file keeps the screen");
        assert_eq!(font(&reopened), font(&app.document));

        app.settings.last_export_directory = None;
        app.settings.export_settings = Default::default();
        let request = app.export_dialog().request().unwrap();
        assert_eq!(request.format, FileFormat::Atascii, "ATASCII is preselected");
        assert_eq!(request.path.extension().unwrap(), mode.extension());
        app.export(&request).unwrap();
        let exported = Document::load(&request.path).unwrap();
        assert_eq!(exported.profile(), ScreenProfile::Atascii(mode), "{:?} loads with its columns", request.path);
        assert_eq!(exported.with_state(|state| state.get_buffer().char_at(Position::new(1, 0)).ch), 'I');
    }
}

#[test]
fn atascii_canvas_keeps_the_width_of_its_screen() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1280.0, 820.0);
    let mut app = DrawApp::new();
    app.create(NewKind::Atascii, Size::new(80, 25));
    app.new_size = [100, 60];
    app.dialog = Some(Dialog::Resize);
    frame(&context, &mut app, size, vec![]);
    frame(&context, &mut app, size, vec![key_event(Key::Enter, egui::Modifiers::NONE)]);
    assert!(app.dialog.is_none());
    assert_eq!(app.document.with_state(|state| state.get_buffer().size()), Size::new(40, 60));
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
        assert!(!rendered("Minimap"), "{size:?}: minimap needs no header");
        assert_eq!(rendered("Layers"), panel, "{size:?}: layers");
        if panel {
            assert!(app.canvas_rect.right() <= size.x - chrome::PANEL_WIDTH, "{size:?}: right panel missing");
            assert!(rendered("80 × 25"), "{size:?}: document dimensions missing");
        }
        assert!(
            (rendered("iCE Colors") || rendered("Blinking") || rendered("Blink"))
                && (rendered("DOS Aspect") || rendered("Square Pixels") || rendered("4:3") || rendered("1:1")),
            "{size:?}: status toggles missing"
        );
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
fn fill_toolbar_only_shows_supported_modes() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.document.tool = Tool::Fill;
    app.document.brush.primary = BrushPrimaryMode::Shading;
    let output = frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    let rendered = |label: &str| {
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) => text.galley.text() == label,
            _ => false,
        })
    };
    assert!(rendered("Character"));
    assert!(rendered("Half Block"));
    assert!(rendered("Colorize"));
    assert!(!rendered("Shade"));
    assert!(!rendered("Replace"));
    assert!(!rendered("Blinking"));
}

#[test]
fn paint_tools_show_hover_preview_with_half_block_height() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.document.tool = Tool::Pencil;
    frame(&context, &mut app, size, vec![]);
    {
        let mut info = app.view.terminal.render_info.write();
        info.display_scale = 2.0;
        info.viewport_width = app.canvas_rect.width();
        info.viewport_height = app.canvas_rect.height();
        info.font_width = 8.0;
        info.font_height = 16.0;
        info.bounds_x = app.canvas_rect.left();
        info.bounds_y = app.canvas_rect.top();
    }
    let info = app.view.terminal.render_info.read().clone();
    let pointer = egui::pos2(
        info.bounds_x + info.viewport_x + info.font_width * info.display_scale * 0.5,
        info.bounds_y + info.viewport_y + info.font_height * info.display_scale * 0.5,
    );
    assert!(
        app.position(pointer).is_some(),
        "preview point {pointer:?} is outside canvas {:?}; render info: {info:?}",
        app.canvas_rect
    );
    let preview_size = |output: &egui::FullOutput| {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.stroke.color == Color32::WHITE && rect.stroke.width == 2.0 && rect.rect.contains(pointer) => Some(rect.rect.size()),
            _ => None,
        })
    };

    app.document.brush.primary = BrushPrimaryMode::Char;
    let mut full = None;
    for tool in [Tool::Pencil, Tool::Fill, Tool::Line, Tool::RectangleOutline, Tool::EllipseFilled] {
        app.document.tool = tool;
        let preview =
            preview_size(&frame(&context, &mut app, size, vec![egui::Event::PointerMoved(pointer)])).unwrap_or_else(|| panic!("{tool:?} hover preview"));
        full.get_or_insert(preview);
    }
    let full = full.unwrap();

    app.document.brush.primary = BrushPrimaryMode::HalfBlock;
    app.document.tool = Tool::Fill;
    let half = preview_size(&frame(&context, &mut app, size, vec![egui::Event::PointerMoved(pointer)])).expect("half-block fill preview");

    assert_eq!(half.x, full.x);
    assert!((half.y * 2.0 - full.y).abs() <= 1.0, "{half:?} should be half the height of {full:?}");

    app.document.tool = Tool::Line;
    app.document.begin(Position::new(0, 0), icy_engine::MouseButton::Left);
    assert!(preview_size(&frame(&context, &mut app, size, vec![egui::Event::PointerMoved(pointer)])).is_none());
    app.document.cancel();
}

#[test]
fn canvas_context_menu_opens_for_text_and_selection_tools() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![]);
    {
        let mut info = app.view.terminal.render_info.write();
        info.display_scale = 2.0;
        info.viewport_width = app.canvas_rect.width();
        info.viewport_height = app.canvas_rect.height();
        info.font_width = 8.0;
        info.font_height = 16.0;
        info.bounds_x = app.canvas_rect.left();
        info.bounds_y = app.canvas_rect.top();
    }
    let info = app.view.terminal.render_info.read().clone();
    let cell = |x: f32, y: f32| {
        egui::pos2(
            info.bounds_x + info.viewport_x + info.font_width * info.display_scale * (x + 0.5),
            info.bounds_y + info.viewport_y + info.font_height * info.display_scale * (y + 0.5),
        )
    };
    let right_click = |app: &mut DrawApp, position: egui::Pos2| {
        frame(&context, app, size, vec![egui::Event::PointerMoved(position)]);
        for pressed in [true, false] {
            frame(
                &context,
                app,
                size,
                vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        frame(&context, app, size, vec![])
    };
    let menu_open = |output: &egui::FullOutput| text_position(output, "Select All").is_some() && text_position(output, "Paste").is_some();
    let caret = |app: &DrawApp| app.document.with_state(|state| state.get_caret().position());

    let output = right_click(&mut app, cell(5.0, 3.0));
    assert!(menu_open(&output), "the text tool opens the context menu");
    assert_eq!(caret(&app), Position::new(5, 3), "the caret moves to the clicked cell");
    click_text(&context, &mut app, size, "Select All");
    assert!(app.document.with_state(|state| state.is_something_selected()));

    frame(&context, &mut app, size, vec![]);
    let output = right_click(&mut app, cell(10.0, 8.0));
    assert!(menu_open(&output), "a right click in the selection opens the context menu");
    assert_eq!(caret(&app), Position::new(5, 3), "inside the selection the caret stays");
    assert!(app.document.with_state(|state| state.is_something_selected()), "the selection is kept");
    click_text(&context, &mut app, size, "Deselect");
    assert!(!app.document.with_state(|state| state.is_something_selected()));

    app.document.tool = Tool::Select;
    app.select_all();
    let output = right_click(&mut app, cell(2.0, 2.0));
    assert!(menu_open(&output), "the selection tool opens the context menu");
    assert!(
        app.document.with_state(|state| state.is_something_selected()),
        "the right button does not start a new selection"
    );
    frame(&context, &mut app, size, vec![key_event(Key::Escape, egui::Modifiers::NONE)]);

    app.document.tool = Tool::Pencil;
    let output = right_click(&mut app, cell(4.0, 4.0));
    assert!(!menu_open(&output), "painting tools keep the right button for the swapped colors");
}

#[test]
fn select_tool_shows_handle_cursors_and_the_add_mode() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![]);
    {
        let mut info = app.view.terminal.render_info.write();
        info.display_scale = 2.0;
        info.viewport_width = app.canvas_rect.width();
        info.viewport_height = app.canvas_rect.height();
        info.font_width = 8.0;
        info.font_height = 16.0;
        info.bounds_x = app.canvas_rect.left();
        info.bounds_y = app.canvas_rect.top();
    }
    let info = app.view.terminal.render_info.read().clone();
    let cell = |x: f32, y: f32| {
        egui::pos2(
            info.bounds_x + info.viewport_x + info.font_width * info.display_scale * (x + 0.5),
            info.bounds_y + info.viewport_y + info.font_height * info.display_scale * (y + 0.5),
        )
    };
    let hover = |app: &mut DrawApp, position: egui::Pos2, modifiers: egui::Modifiers| {
        let time = context.input(|input| input.time) + 0.05;
        context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events: vec![egui::Event::PointerMoved(position)],
                modifiers,
                time: Some(time),
                ..Default::default()
            },
            |context| app.show(context),
        )
    };
    app.document.tool = Tool::Select;
    app.document
        .with_state(|state| state.set_selection(Selection::from(icy_engine::Rectangle::from(5, 3, 10, 8))))
        .unwrap();

    let cursor = |output: egui::FullOutput| output.platform_output.cursor_icon;
    assert_eq!(cursor(hover(&mut app, cell(9.0, 6.0), egui::Modifiers::NONE)), egui::CursorIcon::Move);
    assert_eq!(cursor(hover(&mut app, cell(5.0, 3.0), egui::Modifiers::NONE)), egui::CursorIcon::ResizeNwSe);
    assert_eq!(
        cursor(hover(&mut app, cell(5.0, 6.0), egui::Modifiers::NONE)),
        egui::CursorIcon::ResizeHorizontal
    );
    assert_eq!(cursor(hover(&mut app, cell(9.0, 3.0), egui::Modifiers::NONE)), egui::CursorIcon::ResizeVertical);
    assert_eq!(cursor(hover(&mut app, cell(30.0, 15.0), egui::Modifiers::NONE)), egui::CursorIcon::Default);

    let output = hover(&mut app, cell(9.0, 6.0), egui::Modifiers::SHIFT);
    assert!(text_position(&output, "Add to selection").is_some());
    assert_eq!(cursor(output), egui::CursorIcon::Default, "shift starts a new selection instead of moving");
    let output = hover(&mut app, cell(9.0, 6.0), egui::Modifiers::CTRL);
    assert!(text_position(&output, "Remove from selection").is_some());
    assert!(text_position(&hover(&mut app, cell(9.0, 6.0), egui::Modifiers::NONE), "5, 3  ·  10 × 8").is_some());

    app.document.selection_mode = icy_draw::document::SelectionMode::Character;
    assert_eq!(
        cursor(hover(&mut app, cell(9.0, 6.0), egui::Modifiers::NONE)),
        egui::CursorIcon::Default,
        "matching modes do not drag the rectangle"
    );
}

#[test]
fn toolbar_color_switcher_swaps_and_opens_palette_popup() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let rendered = |output: &egui::FullOutput, label: &str| {
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) => text.galley.text().contains(label),
            _ => false,
        })
    };
    let origin = std::cell::Cell::new(egui::Pos2::ZERO);
    let draw = |app: &mut DrawApp, events: Vec<egui::Event>| {
        context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 100.0))),
                events,
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    origin.set(ui.cursor().min);
                    app.color_switcher(ui);
                });
            },
        )
    };
    draw(&mut app, vec![]);
    let swap = origin.get() + egui::vec2(33.0, 7.0);
    let colors = |app: &DrawApp| {
        app.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (attribute.foreground(), attribute.background())
        })
    };
    let (foreground, background) = colors(&app);
    draw(&mut app, pointer(swap, true));
    draw(&mut app, pointer(swap, false));
    assert_eq!(colors(&app), (background, foreground));
    let swatch = origin.get() + egui::vec2(10.0, 10.0);
    draw(&mut app, pointer(swatch, true));
    draw(&mut app, pointer(swatch, false));
    let output = draw(&mut app, vec![]);
    assert!(rendered(&output, "Edit Palette"), "palette popup missing");
}

#[test]
fn pipette_toolbar_previews_hovered_character_and_colors() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.document.tool = Tool::Pipette;
    app.document.with_state(|state| {
        state.get_buffer_mut().layers[0].set_char(Position::default(), icy_engine::AttributedChar::new('#', icy_engine::TextAttribute::new(11, 0)));
    });
    app.pipette_hover = Some((Position::default(), egui::Modifiers::NONE));

    let output = context.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 100.0))),
            ..Default::default()
        },
        |context| {
            egui::CentralPanel::default().show(context, |ui| {
                ui.horizontal_centered(|ui| app.pipette_options(ui));
            });
        },
    );
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect();
    for expected in ["#23", "FG 11", "#55FFFF", "BG 0", "#000000"] {
        assert!(labels.iter().any(|label| label.contains(expected)), "missing {expected:?} in {labels:?}");
    }
}

#[test]
fn font_filter_highlights_all_case_insensitive_matches() {
    assert_eq!(filter_match_ranges("Blue blue BLUE", "blue"), vec![0..4, 5..9, 10..14]);
    assert!(filter_match_ranges("1911", "").is_empty());
}

#[test]
fn text_art_font_dialog_groups_color_variants_into_one_row() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    let font = |name: &str| retrofont::Font::Tdf(Box::new(retrofont::tdf::TdfFont::new(name, icy_engine_edit::charset::TdfFontType::Color, 1)));
    app.text_fonts = Some(icy_draw::text_art_fonts::TextArtFontLibrary::with_fonts(vec![
        font("Acidscape1C"),
        font("Acidscape1G"),
        font("Acidscape1R"),
        font("Blade"),
    ]));
    app.dialog = Some(Dialog::TextArtFontSelect);
    frame(&context, &mut app, size, vec![]);
    let texts = |output: &egui::FullOutput| -> Vec<(String, egui::Pos2)> {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((text.galley.text().to_owned(), text.pos + text.galley.size() / 2.0)),
                _ => None,
            })
            .collect()
    };
    let output = texts(&frame(&context, &mut app, size, vec![]));
    let shown = |name: &str| output.iter().any(|(text, _)| text == name);
    assert!(shown("Acidscape1C") && shown("Blade"));
    assert!(!shown("Acidscape1G") && !shown("Acidscape1R"), "variants need their own rows");
    for label in ["C", "G", "R"] {
        assert!(shown(label), "missing variant chip {label}");
    }

    let chip = output.iter().find(|(text, _)| text == "G").unwrap().1;
    frame(&context, &mut app, size, pointer(chip, true));
    frame(&context, &mut app, size, pointer(chip, false));
    assert_eq!(app.text_font_pending, 1);
    let output = texts(&frame(&context, &mut app, size, vec![]));
    assert!(output.iter().any(|(text, _)| text == "Acidscape1G"));
    assert!(!output.iter().any(|(text, _)| text == "Acidscape1C"));
}

#[test]
fn favorite_variant_makes_its_whole_group_a_favorite() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    let font = |name: &str| retrofont::Font::Tdf(Box::new(retrofont::tdf::TdfFont::new(name, icy_engine_edit::charset::TdfFontType::Color, 1)));
    app.text_fonts = Some(icy_draw::text_art_fonts::TextArtFontLibrary::with_fonts(vec![
        font("Acidscape1C"),
        font("Acidscape1G"),
        font("Acidscape1R"),
        font("Blade"),
    ]));
    app.settings.text_art_font_favorites = vec!["Color:Acidscape1G".to_owned()];
    app.text_font_favorites_only = true;
    app.text_font_pending = 3;
    app.dialog = Some(Dialog::TextArtFontSelect);
    frame(&context, &mut app, size, vec![]);
    let output = frame(&context, &mut app, size, vec![]);
    let texts: Vec<(String, egui::Pos2)> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some((text.galley.text().to_owned(), text.pos + text.galley.size() / 2.0)),
            _ => None,
        })
        .collect();
    let shown = |name: &str| texts.iter().any(|(text, _)| text == name);
    assert!(shown("Acidscape1G"), "the favorite variant is shown for its group");
    assert!(!shown("Blade"));
    for label in ["C", "G", "R", "★"] {
        assert!(shown(label), "missing {label}");
    }

    let star = texts.iter().find(|(text, _)| text == "★").unwrap().1;
    frame(&context, &mut app, size, pointer(star, true));
    frame(&context, &mut app, size, pointer(star, false));
    assert!(app.settings.text_art_font_favorites.is_empty(), "the star unfavorites the whole group");
}

#[test]
fn text_art_font_dialog_scrolls_the_current_font_into_the_middle() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    let font = |name: String| retrofont::Font::Tdf(Box::new(retrofont::tdf::TdfFont::new(name, icy_engine_edit::charset::TdfFontType::Color, 1)));
    let mut fonts: Vec<_> = (0..400).map(|number| font(format!("Font {number:03}"))).collect();
    fonts.extend(["Acidscape1C", "Acidscape1G", "Acidscape1R"].map(|name| font(name.to_owned())));
    app.text_fonts = Some(icy_draw::text_art_fonts::TextArtFontLibrary::with_fonts(fonts));
    app.document.tool = Tool::Font;
    let positions = |output: &egui::FullOutput, name: &str| -> Vec<egui::Pos2> {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == name => Some(text.pos + text.galley.size() / 2.0),
                _ => None,
            })
            .collect()
    };
    for (current, name) in [(250, "Font 250"), (398, "Font 398"), (402, "Acidscape1R")] {
        app.text_font = current;
        let output = frame(&context, &mut app, size, vec![]);
        let button = positions(&output, name)[0];
        frame(&context, &mut app, size, pointer(button, true));
        frame(&context, &mut app, size, pointer(button, false));
        frame(&context, &mut app, size, vec![]);
        let output = frame(&context, &mut app, size, vec![]);
        assert!(matches!(app.dialog, Some(Dialog::TextArtFontSelect)));
        assert_eq!(app.text_font_pending, current);
        let rows = positions(&output, name);
        assert!(
            rows.iter().any(|row| (200.0..650.0).contains(&row.y)),
            "{name} is not scrolled into view: {rows:?}"
        );
        app.dialog = None;
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

pub(super) fn key_event(key: Key, modifiers: egui::Modifiers) -> egui::Event {
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
fn text_tool_keeps_canvas_focus_after_arrow_keys() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.document.tool = Tool::Click;
    for _ in 0..2 {
        frame(&context, &mut app, size, vec![]);
    }
    let position = app.canvas_rect.center();
    for pressed in [true, false] {
        frame(&context, &mut app, size, pointer(position, pressed));
    }
    frame(&context, &mut app, size, vec![]);
    let canvas = context.memory(|memory| memory.focused()).expect("clicking the canvas gives it keyboard focus");
    app.document.with_state(|state| state.set_caret_position((4, 4).into()));
    frame(&context, &mut app, size, vec![egui::Event::Text("A".into())]);
    for (key, expected) in [
        (Key::ArrowLeft, Position::new(4, 4)),
        (Key::ArrowUp, Position::new(4, 3)),
        (Key::ArrowRight, Position::new(5, 3)),
        (Key::ArrowDown, Position::new(5, 4)),
    ] {
        frame(&context, &mut app, size, vec![key_event(key, egui::Modifiers::NONE)]);
        frame(&context, &mut app, size, vec![]);
        assert_eq!(
            context.memory(|memory| memory.focused()),
            Some(canvas),
            "{key:?} moved keyboard focus away from the canvas"
        );
        assert!(app.canvas_focus);
        assert_eq!(app.document.with_state(|state| state.get_caret().position()), expected);
    }
    frame(&context, &mut app, size, vec![egui::Event::Text("B".into())]);
    app.document.with_state(|state| {
        assert_eq!(state.get_buffer().char_at((4, 4).into()).ch, 'A');
        assert_eq!(state.get_buffer().char_at((5, 4).into()).ch, 'B');
    });
}

/// Picking a color or F-key set in the chrome must leave the keyboard on the canvas.
#[test]
fn canvas_keeps_keyboard_after_toolbar_and_palette_clicks() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    context.style_mut(|style| style.animation_time = 0.0);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.document.tool = Tool::Click;
    for _ in 0..2 {
        frame(&context, &mut app, size, vec![]);
    }
    let canvas = app.canvas_rect.center();
    for pressed in [true, false] {
        frame(&context, &mut app, size, pointer(canvas, pressed));
    }
    let output = frame(&context, &mut app, size, vec![]);
    let set_label = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with("Set ") => Some(text.pos + egui::vec2(0.0, text.galley.size().y / 2.0)),
            _ => None,
        })
        .expect("F-key set label");
    let (red, green, blue) = app.document.with_state(|state| state.get_buffer().palette.rgb(4));
    let swatch = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == Color32::from_rgb(red, green, blue) && rect.rect.left() < app.canvas_rect.left() => {
                Some(rect.rect.center())
            }
            _ => None,
        })
        .expect("sidebar palette swatch");
    let next_set = set_label - egui::vec2(16.0, 0.0);
    let first_set = app.settings.fkeys.current_set;
    let toolbar_swatch = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.rect.size() == egui::Vec2::splat(24.0) && rect.rect.top() < app.canvas_rect.top() => Some(rect.rect.center()),
            _ => None,
        })
        .expect("toolbar color switcher");

    for (name, targets) in [
        ("palette swatch", vec![swatch]),
        ("next F-key set", vec![next_set]),
        ("toolbar palette popup", vec![toolbar_swatch, egui::Pos2::ZERO]),
    ] {
        for mut target in targets {
            if target == egui::Pos2::ZERO {
                let output = frame(&context, &mut app, size, vec![]);
                let (red, green, blue) = app.document.with_state(|state| state.get_buffer().palette.rgb(2));
                target = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect) if rect.fill == Color32::from_rgb(red, green, blue) => Some(rect.rect.center()),
                        _ => None,
                    })
                    .find(|point| point.x > app.canvas_rect.left())
                    .expect("popup palette swatch");
            }
            for pressed in [true, false] {
                frame(&context, &mut app, size, pointer(target, pressed));
            }
            frame(&context, &mut app, size, vec![]);
        }
        app.document.with_state(|state| state.set_caret_position((2, 2).into()));
        frame(&context, &mut app, size, vec![egui::Event::Text("A".into())]);
        frame(&context, &mut app, size, vec![key_event(Key::ArrowDown, egui::Modifiers::NONE)]);
        frame(&context, &mut app, size, vec![key_event(Key::ArrowLeft, egui::Modifiers::NONE)]);
        frame(&context, &mut app, size, vec![egui::Event::Text("B".into())]);
        app.document.with_state(|state| {
            assert_eq!(state.get_buffer().char_at((2, 2).into()).ch, 'A', "typing after clicking the {name}");
            assert_eq!(state.get_buffer().char_at((2, 3).into()).ch, 'B', "arrow keys after clicking the {name}");
        });
    }
    assert_eq!(app.document.with_state(|state| state.get_caret().attribute.foreground()), 2);
    assert_ne!(app.settings.fkeys.current_set, first_set);
}

#[test]
fn text_tool_yields_to_focused_text_fields() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let mut value = String::new();
    let field = egui::Id::new("focus-test-field");
    for (index, events) in [
        vec![],
        vec![egui::Event::Text("xy".into())],
        vec![key_event(Key::ArrowLeft, egui::Modifiers::NONE)],
        vec![egui::Event::Text("Z".into())],
    ]
    .into_iter()
    .enumerate()
    {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                events,
                ..Default::default()
            },
            |context| {
                egui::TopBottomPanel::top("focus-test-panel").show(context, |ui| {
                    let response = ui.add(egui::TextEdit::singleline(&mut value).id(field));
                    if index == 0 {
                        response.request_focus();
                    }
                });
                app.show(context);
            },
        );
        assert_eq!(context.memory(|memory| memory.focused()), Some(field));
        assert!(!app.canvas_focus);
    }
    assert_eq!(value, "xZy");
    assert!(!app.document.modified(), "typing into a focused field must not edit the canvas");
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
fn animation_export_dialog_exports_and_asks_before_overwriting() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let mut animation = super::super::animation::AnimationEditor::new();
    animation.source = "local screen = new_buffer(10, 3)\nnext_frame(screen)\nnext_frame(screen)".into();
    animation.compile_for_test();
    app.animation = Some(animation);
    let size = egui::vec2(1280.0, 820.0);
    frame(
        &context,
        &mut app,
        size,
        vec![key_event(Key::E, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)],
    );
    assert!(app.animation.as_ref().unwrap().export_dialog_open(), "Ctrl+Shift+E opens the export dialog");
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("demo");
    app.animation.as_mut().unwrap().set_export_path(target.clone());
    let export = icy_draw::fl!("menu-export").trim_end_matches(['…', '.']).to_string();
    let click_export = |app: &mut DrawApp| {
        let mut output = frame(&context, app, size, vec![]);
        for _ in 0..3 {
            output = frame(&context, app, size, vec![]);
        }
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((text.galley.text().to_string(), text.visual_bounding_rect())),
                _ => None,
            })
            .collect();
        let (_, rect) = labels.iter().rev().find(|(label, _)| *label == export).expect("export button");
        for pressed in [true, false] {
            frame(&context, app, size, pointer(rect.center(), pressed));
        }
    };
    click_export(&mut app);
    let gif = target.with_extension("gif");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.animation.as_ref().unwrap().export_dialog_open() {
        assert!(std::time::Instant::now() < deadline, "export did not finish");
        frame(&context, &mut app, size, vec![]);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(image::open(&gif).is_ok(), "a GIF was written");
    app.animation.as_mut().unwrap().open_export_dialog();
    app.animation.as_mut().unwrap().set_export_path(gif.clone());
    click_export(&mut app);
    assert!(matches!(app.dialog, Some(Dialog::AnimationOverwrite(ref path, _)) if *path == gif));
}

#[test]
fn tag_draft_cancel_does_not_create_or_change_a_tag() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.open_tag_properties(None);
    // A modal dialog only takes keyboard input once it is the top modal, from its second frame.
    for _ in 0..2 {
        frame(&context, &mut app, size, vec![]);
    }
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
        for label in ["Characters".to_owned(), labels::cancel()] {
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
fn export_dialog_has_no_header_or_close_glyph() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.dialog = Some(Dialog::Export);
    for _ in 0..3 {
        frame(&context, &mut app, egui::vec2(900.0, 700.0), vec![]);
    }
    let output = frame(&context, &mut app, egui::vec2(900.0, 700.0), vec![]);
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect();
    let export = icy_engine_gui::egui::dialog::labels::export();
    assert_eq!(
        labels.iter().filter(|label| **label == export).count(),
        1,
        "only the button names the action: {labels:?}"
    );
    assert!(!labels.contains(&"×"));
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
        app.attribute_picker = Some(0);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "attribute-picker-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "attribute-picker");
        app.attribute_picker = None;
        app.dialog = Some(Dialog::Characters);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "characters-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "characters");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "characters-compact-warmup");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "characters-compact");
        app.open_palette_editor(true);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "palette-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "palette");
        let mut large = icy_engine::Palette::dos_default();
        large.resize(256);
        app.palette_editor = super::super::palette::PaletteEditor::new(large, 200);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "palette-256-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "palette-256");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "palette-compact-warmup");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "palette-compact");
        app.dialog = None;
        app.show_start = true;
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            gpu.context.set_theme(theme);
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], "start-warmup");
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], &format!("start-{theme:?}"));
        }
        gpu.context.set_theme(egui::Theme::Dark);
        app.show_start = false;
        for (page, name) in [
            (settings_dialog::Page::Monitor, "settings-monitor"),
            (settings_dialog::Page::FontOutline, "settings-outline"),
            (settings_dialog::Page::Charset, "settings-charset"),
            (settings_dialog::Page::Paths, "settings-paths"),
        ] {
            let mut draft = settings_dialog::SettingsDraft::new(&app.settings);
            draft.page = page;
            draft.slot = Some(2);
            app.dialog = Some(Dialog::Settings(Box::new(draft)));
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], &format!("{name}-warmup"));
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], name);
        }
        app.dialog = None;
        app.open_layer_properties(0);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "layer-properties-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "layer-properties");
        app.chrome = Default::default();
        app.open_font_selector();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "font-select-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "font-select");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "font-select-compact-warmup");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "font-select-compact");
        app.dialog = None;
        let font = app.document.with_state(|state| state.get_buffer().font(0).unwrap().clone());
        let mut editor = super::super::font::FontEditor::new(font);
        editor.apply_target = true;
        app.font_editor = Some(editor);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "font-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "font");
        {
            use icy_engine_edit::bitfont::BitFontFocusedPanel;
            let state = &mut app.font_editor.as_mut().unwrap().state;
            state.set_selection(Some((1, 2, 4, 6)));
            state.set_charset_selection(Some((icy_engine::Position::new(14, 4), icy_engine::Position::new(3, 5), false)));
            state.set_charset_cursor(3, 5);
            state.set_focused_panel(BitFontFocusedPanel::CharSet);
        }
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "font-selection");
        gpu.capture(&mut app, [900, 600], 1.0, vec![], "font-small-warmup");
        gpu.capture(&mut app, [900, 600], 1.0, vec![], "font-small");
        app.font_editor = None;
        let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
        app.replace(font.document()); app.charfont = Some(font);
        app.document.type_text("TDF").unwrap();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tdf-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tdf");
        let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Outline);
        app.replace(font.document()); app.charfont = Some(font);
        app.document.type_text("ABCDEFGHIJKLMNOPQ@&").unwrap();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "outline-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "outline");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "outline-compact-warmup");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "outline-compact");
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
        let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/icy_animations/mandel.icyanim");
        let mut mandel = super::super::animation::AnimationEditor::load(&sample).unwrap();
        mandel.compile_for_test();
        mandel.select_frame_for_test(4);
        app.animation = Some(mandel);
        for (size, name) in [([1280, 820], "animation-mandel"), ([440, 700], "animation-mandel-compact")] {
            gpu.capture(&mut app, size, 1.0, vec![], "animation-mandel-warmup");
            gpu.capture(&mut app, size, 1.0, vec![], name);
        }
        app.animation.as_mut().unwrap().show_log_for_test(true);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "animation-log-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "animation-log");
        app.animation.as_mut().unwrap().show_log_for_test(false);
        app.animation.as_mut().unwrap().open_export_dialog();
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "animation-export-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "animation-export");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "animation-export-compact-warmup");
        gpu.capture(&mut app, [440, 700], 1.0, vec![], "animation-export-compact");
    });
}

pub(super) struct Gpu {
    context: egui::Context,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
}

impl Gpu {
    pub(super) async fn new() -> Self {
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

    pub(super) fn capture(&mut self, app: &mut DrawApp, size: [u32; 2], scale: f32, events: Vec<egui::Event>, name: &str) -> Vec<u8> {
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

#[test]
fn classic_menu_commands_edit_colors_and_markers() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![]);

    app.document.type_text("AB").unwrap();
    app.document.with_state(|state| state.set_caret_position(Position::new(0, 0)));
    frame(
        &context,
        &mut app,
        size,
        vec![key_event(Key::ArrowUp, egui::Modifiers::ALT), egui::Event::Text("∆".into())],
    );
    let char_at = |app: &mut DrawApp, x, y| app.document.with_state(|state| state.get_buffer().char_at(Position::new(x, y)).ch);
    assert_eq!(char_at(&mut app, 0, 1), 'A', "Alt+Up inserts a row above the caret");
    assert_eq!(char_at(&mut app, 0, 0), ' ', "the Option text of a consumed Alt shortcut is not typed");
    app.area_operation(menus::AreaOp::DeleteRow);
    assert_eq!(char_at(&mut app, 0, 0), 'A');

    let colors = |app: &mut DrawApp| {
        app.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (attribute.foreground(), attribute.background())
        })
    };
    let (foreground, background) = colors(&mut app);
    frame(&context, &mut app, size, vec![key_event(Key::ArrowDown, egui::Modifiers::COMMAND)]);
    assert_eq!(colors(&mut app), (foreground + 1, background));
    frame(&context, &mut app, size, vec![key_event(Key::ArrowLeft, egui::Modifiers::COMMAND)]);
    let count = app.document.with_state(|state| state.get_buffer().palette.len() as u32);
    assert_eq!(colors(&mut app), (foreground + 1, (background + count - 1) % count));
    app.color_operation(menus::ColorOp::Default);
    assert_eq!(colors(&mut app), (foreground, background));
    app.color_operation(menus::ColorOp::Swap);
    assert_eq!(colors(&mut app), (background, foreground));

    app.guide = Some((80, 25));
    app.raster = Some((8, 4));
    frame(&context, &mut app, size, vec![]);
    let font = app.document.with_state(|state| state.get_buffer().font_dimensions());
    let markers = app.view.markers.clone().unwrap();
    assert_eq!(markers.guide, Some((80.0 * font.width as f32, 25.0 * font.height as f32)));
    assert_eq!(markers.raster, Some((8.0 * font.width as f32, 4.0 * font.height as f32)));
    frame(&context, &mut app, size, vec![key_event(Key::Semicolon, egui::Modifiers::COMMAND)]);
    assert!(!app.show_guide);
    frame(&context, &mut app, size, vec![]);
    assert!(app.view.markers.as_ref().unwrap().guide.is_none(), "Cmd+; hides the guide");

    app.show_line_numbers = false;
    frame(&context, &mut app, size, vec![key_event(Key::R, egui::Modifiers::COMMAND)]);
    assert!(app.show_line_numbers);
    frame(&context, &mut app, size, vec![]);
}

#[test]
fn insert_image_accepts_all_writable_art_formats() {
    use icy_engine::{AttributedChar, FileFormat, TextBuffer, TextPane};

    let directory = tempfile::tempdir().unwrap();
    let mut source = TextBuffer::create((80, 2));
    source.ice_mode = icy_engine::IceMode::Ice;
    source.layers[0].set_char((0, 0), AttributedChar::new('A', Default::default()));
    for format in FileFormat::ALL.iter().filter(|format| format.supports_load() && format.supports_save()) {
        let bytes = if matches!(format, FileFormat::Petscii | FileFormat::Atascii | FileFormat::Vt52) {
            b"HELLO".to_vec()
        } else {
            format
                .to_bytes(&source, &icy_engine::SaveOptions::default())
                .unwrap_or_else(|error| panic!("{format}: {error}"))
        };
        let path = directory.path().join(format!("art.{}", format.primary_extension()));
        std::fs::write(&path, bytes).unwrap();
        let loaded = icy_draw::files::load_insert_art(&path).unwrap();
        let mut app = DrawApp::new();
        let count = app.document.with_state(|state| state.get_buffer().layers.len());
        app.insert_image(&path);
        if let Some(Dialog::Error(error)) = &app.dialog {
            panic!("{format}: {error}");
        }
        app.document.with_state(|state| {
            assert_eq!(state.get_buffer().layers.len(), count + 1, "{format}");
            let ch = state.get_cur_layer().unwrap().char_at((0, 0).into());
            let original = loaded.char_at((0, 0).into());
            assert_eq!(ch.ch, original.ch, "{format}: glyph indices must not change");
            assert_eq!(
                state.get_buffer().font(ch.font_page()),
                loaded.font(original.font_page()),
                "{format}: font must survive"
            );
        });
        app.document.undo().unwrap();
        assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), count, "{format}");
    }
}

#[test]
fn insert_image_decodes_sixel_and_qoi_files() {
    let directory = tempfile::tempdir().unwrap();
    let sixel = directory.path().join("image.SIX");
    std::fs::write(&sixel, b"\x1bPq\"1;1;2;6#0;2;100;0;0#0~~\x1b\\").unwrap();
    let qoi = directory.path().join("image.qoi");
    image::RgbaImage::from_pixel(3, 4, image::Rgba([255, 0, 0, 255])).save(&qoi).unwrap();
    for path in [&sixel, &qoi] {
        let mut app = DrawApp::new();
        app.insert_image(path);
        if let Some(Dialog::Error(error)) = &app.dialog {
            panic!("{}: {error}", path.display());
        }
        assert!(app.document.paste_active());
        assert_eq!(app.document.with_state(|state| state.get_cur_layer().unwrap().sixels.len()), 1);
    }
}

#[test]
fn insert_image_accepts_ansi_as_a_new_editable_layer() {
    let mut app = DrawApp::new();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("art.ANS");
    std::fs::write(&path, b"\x1b[31mHELLO\x1b[0m\r\nWORLD").unwrap();
    app.document.with_state(|state| state.set_caret_position((4, 3).into()));
    let original = app.document.with_state(|state| state.get_buffer().layers.len());
    app.insert_image(&path);
    assert!(app.dialog.is_none());
    assert!(
        !app.document.paste_active(),
        "artwork stays on a new layer instead of being anchored into the old one"
    );
    app.document.with_state(|state| {
        assert_eq!(state.get_buffer().layers.len(), original + 1);
        let layer = state.get_cur_layer().unwrap();
        assert_eq!(layer.role, icy_engine::Role::Normal);
        assert_eq!(layer.offset(), icy_engine::Position::new(4, 3));
        assert_eq!(layer.char_at((0, 0).into()).ch, 'H');
        assert_eq!(layer.char_at((0, 1).into()).ch, 'W');
        assert_eq!(layer.char_at((0, 0).into()).attribute.foreground(), 4);
    });
    app.document.undo().unwrap();
    assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), original);
    app.document.redo().unwrap();
    assert_eq!(app.document.with_state(|state| state.get_cur_layer().unwrap().char_at((0, 0).into()).ch), 'H');
}

#[test]
fn insert_image_flattens_visible_icy_layers_without_replacing_document_colors() {
    use icy_engine::{AttributeColor, AttributedChar, FileFormat, Layer, TextBuffer, TextPane};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("layers.icy");
    let mut source = TextBuffer::create((5, 2));
    source.layers[0].set_char((0, 0), AttributedChar::new('A', Default::default()));
    source.palette.set_color_rgb(7, 12, 34, 56);
    let mut overlay = Layer::new("Overlay", (2, 1));
    overlay.properties.has_alpha_channel = true;
    overlay.set_offset((1, 0));
    overlay.set_char((0, 0), AttributedChar::new('B', Default::default()));
    source.layers.push(overlay);
    let mut hidden = Layer::new("Hidden", (5, 2));
    hidden.properties.is_visible = false;
    hidden.set_char((0, 0), AttributedChar::new('X', Default::default()));
    source.layers.push(hidden);
    let font = icy_engine::BitFont::from_basic(8, 16, &[0x55; 256 * 16]);
    source.set_font(0, font.clone());
    std::fs::write(&path, FileFormat::IcyDraw.to_bytes(&source, &icy_engine::SaveOptions::icy_draw()).unwrap()).unwrap();
    let mut app = DrawApp::new();
    let palette = app.document.with_state(|state| state.get_buffer().palette.clone());
    let count = app.document.with_state(|state| state.get_buffer().layers.len());
    let fonts = app.document.with_state(|state| state.get_buffer().font_table());
    app.insert_image(&path);
    assert!(app.dialog.is_none());
    app.document.with_state(|state| {
        assert_eq!(state.get_buffer().layers.len(), count + 1);
        assert!(state.get_buffer().palette.are_colors_equal(&palette));
        let layer = state.get_cur_layer().unwrap();
        assert_eq!(layer.size(), icy_engine::Size::new(5, 2));
        assert_eq!(layer.char_at((0, 0).into()).ch, 'A');
        assert_eq!(layer.char_at((1, 0).into()).ch, 'B');
        assert_eq!(layer.char_at((0, 0).into()).attribute.foreground_color(), AttributeColor::Rgb(12, 34, 56));
        assert_eq!(state.get_buffer().font(layer.char_at((0, 0).into()).font_page()), Some(&font));
    });
    app.document.undo().unwrap();
    assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), count);
    assert_eq!(app.document.with_state(|state| state.get_buffer().font_table()), fonts);
    app.document.redo().unwrap();
    app.document.with_state(|state| {
        let page = state.get_cur_layer().unwrap().char_at((0, 0).into()).font_page();
        assert_eq!(state.get_buffer().font(page), Some(&font));
    });
}

#[test]
fn insert_image_reports_broken_icy_without_changing_document() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("broken.icy");
    std::fs::write(&path, b"not an icy file").unwrap();
    let mut app = DrawApp::new();
    let layers = app.document.with_state(|state| state.get_buffer().layers.len());
    app.insert_image(&path);
    assert!(matches!(app.dialog, Some(Dialog::Error(_))));
    assert_eq!(app.document.with_state(|state| state.get_buffer().layers.len()), layers);
}

#[test]
fn insert_image_creates_a_floating_image_layer() {
    let mut app = DrawApp::new();
    let path = std::env::temp_dir().join(format!("icy_draw_insert_{}.png", std::process::id()));
    image::RgbaImage::from_pixel(24, 32, image::Rgba([255, 0, 0, 255])).save(&path).unwrap();
    let layers = app.document.with_state(|state| state.get_buffer().layers.len());
    app.insert_image(&path);
    let _ = std::fs::remove_file(&path);
    assert!(app.dialog.is_none());
    assert!(app.document.paste_active());
    let (count, role, sixels) = app.document.with_state(|state| {
        let layer = state.get_cur_layer().unwrap();
        (state.get_buffer().layers.len(), layer.role, layer.sixels.len())
    });
    assert_eq!(count, layers + 1);
    assert_eq!(role, icy_engine::Role::Image);
    assert_eq!(sixels, 1);
}

fn pump_until(context: &egui::Context, app: &mut DrawApp, what: &str, mut done: impl FnMut(&mut DrawApp) -> bool) {
    for _ in 0..300 {
        frame(context, app, egui::vec2(1280.0, 820.0), vec![]);
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("timed out waiting for {what}");
}

fn wait_for_event(
    runtime: &tokio::runtime::Runtime,
    events: &mut tokio::sync::mpsc::Receiver<icy_engine_edit::collaboration::CollaborationEvent>,
    what: &str,
    mut matches: impl FnMut(&icy_engine_edit::collaboration::CollaborationEvent) -> bool,
) {
    let found = runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(6), async {
            while let Some(event) = events.recv().await {
                if matches(&event) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false)
    });
    assert!(found, "peer never received {what}");
}

#[test]
fn collaboration_session_syncs_document_chat_and_cursors() {
    use icy_engine_edit::collaboration::{self as collaboration, Block, ClientConfig, CollaborationEvent, CursorMode, ServerConfig};

    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.spawn(collaboration::run_server(ServerConfig {
        bind_addr: format!("127.0.0.1:{port}").parse().unwrap(),
        columns: 40,
        rows: 10,
        autosave: icy_engine_edit::collaboration::AutosaveConfig {
            backup_folder: std::env::temp_dir(),
            interval: None,
            ..Default::default()
        },
        ..Default::default()
    }));
    let url = format!("127.0.0.1:{port}");
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (peer, mut peer_events) = runtime
        .block_on(collaboration::connect(ClientConfig {
            url: url.clone(),
            nick: "peer".into(),
            ..Default::default()
        }))
        .unwrap();
    wait_for_event(&runtime, &mut peer_events, "its own session", |event| {
        matches!(event, CollaborationEvent::Connected(_))
    });

    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.open_connect_dialog();
    assert!(matches!(app.dialog, Some(Dialog::Connect)));
    app.dialog = None;
    app.collab.form.url = url.clone();
    app.collab.form.nick = "me".into();
    app.collab.form.group = "crew".into();
    app.start_collaboration(&context);
    assert!(app.collab.connecting);
    assert!(app.settings.collaboration_servers_list().contains(&url));
    pump_until(&context, &mut app, "the session document", |app| app.collab.active);
    assert!(app.collab.chat_visible);
    assert_eq!(app.document.with_state(|state| state.get_buffer().size()), Size::new(40, 10));
    assert!(app.collab.core.remote_users.values().any(|user| user.user.nick == "peer"));
    assert!(!app.modified(), "the server owns the shared document");
    wait_for_event(
        &runtime,
        &mut peer_events,
        "the join",
        |event| matches!(event, CollaborationEvent::UserJoined(user) if user.nick == "me"),
    );

    frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![egui::Event::Text("A".into())]);
    let local_undo = app.document.with_state(|state| state.undo_stack_len());
    wait_for_event(
        &runtime,
        &mut peer_events,
        "the typed character",
        |event| matches!(event, CollaborationEvent::Draw { col: 0, row: 0, block } if block.code == 'A' as u32),
    );

    runtime
        .block_on(peer.draw(
            5,
            2,
            Block {
                code: 'Z' as u32,
                fg: 4,
                bg: 1,
            },
        ))
        .unwrap();
    runtime.block_on(peer.send_cursor(3, 4)).unwrap();
    runtime.block_on(peer.send_chat("hello there".into())).unwrap();
    pump_until(&context, &mut app, "the peer's edits", |app| {
        app.collab.core.chat_messages.iter().any(|message| message.text == "hello there")
    });
    let ch = app.document.with_state(|state| state.get_buffer().char_at(Position::new(5, 2)));
    assert_eq!((ch.ch, ch.attribute.foreground(), ch.attribute.background()), ('Z', 4, 1));
    let peer_user = app.collab.core.remote_users.values().find(|user| user.user.nick == "peer").unwrap();
    assert_eq!((peer_user.cursor, peer_user.cursor_mode), (Some((3, 4)), CursorMode::Editing));
    assert_eq!(
        app.document.with_state(|state| state.undo_stack_len()),
        local_undo,
        "remote edits are not undoable locally"
    );

    runtime.block_on(peer.set_canvas_size(50, 12)).unwrap();
    pump_until(&context, &mut app, "the remote resize", |app| {
        app.document.with_state(|state| state.get_buffer().size()) == Size::new(50, 12)
    });
    assert!(app
        .collab
        .core
        .chat_messages
        .iter()
        .any(|message| message.text.contains("changed the canvas size to 50 × 12")));

    app.document.start_paste("XY", None).unwrap();
    frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    wait_for_event(
        &runtime,
        &mut peer_events,
        "the floating paste",
        |event| matches!(event, CollaborationEvent::PasteAsSelection { blocks, .. } if blocks.data[0].code == 'X' as u32 && blocks.data[1].code == 'Y' as u32),
    );
    app.document.paste_action(icy_draw::document::PasteAction::Cancel).unwrap();
    frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);

    app.collab.chat_input = "hi peer".into();
    app.collab.send_chat();
    assert!(app
        .collab
        .core
        .chat_messages
        .iter()
        .any(|message| message.text == "hi peer" && message.nick == "me"));
    wait_for_event(
        &runtime,
        &mut peer_events,
        "the chat message",
        |event| matches!(event, CollaborationEvent::Chat(message) if message.text == "hi peer"),
    );

    app.disconnect_collaboration();
    assert!(!app.collab.in_session());
    wait_for_event(&runtime, &mut peer_events, "the leave", |event| {
        matches!(event, CollaborationEvent::UserLeft { .. })
    });
}

#[test]
fn connecting_offers_to_save_unsaved_work_first() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.document.type_text("KEEP").unwrap();
    app.open_connect_dialog();
    assert!(matches!(app.dialog, Some(Dialog::Close)));
    app.dialog = None;
    app.complete_close(&context);
    assert!(matches!(app.dialog, Some(Dialog::Connect)), "discarding continues to the connect dialog");
}

#[test]
fn outline_digits_type_the_thedraw_codes() {
    let mut app = DrawApp::new();
    let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Outline);
    app.replace(font.document());
    app.charfont = Some(font);
    app.document.type_text("a15678").unwrap();
    let typed: String = app
        .document
        .with_state(|state| (0..6).map(|x| state.get_buffer().char_at(Position::new(x, 0)).ch).collect());
    assert_eq!(typed, "AKO@&\u{ff}");
}

#[test]
fn tdf_editor_has_no_layer_panel() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
    app.replace(font.document());
    app.charfont = Some(font);
    let size = egui::vec2(1280.0, 820.0);
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    let output = frame(&context, &mut app, size, vec![]);
    let rendered = |label: &str| {
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) => text.galley.text() == label,
            _ => false,
        })
    };
    assert!(rendered(&icy_draw::fl!("new-file-editor-tdf")), "TDF font section missing");
    assert!(!rendered(&icy_draw::fl!("layer_tool_title")), "layer list shown in the TDF editor");
}

#[test]
fn chat_panel_keeps_its_default_height() {
    use icy_engine_edit::collaboration::ChatMessage;

    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    app.collab.active = true;
    app.collab.chat_visible = true;
    for index in 0..40 {
        app.collab.core.add_chat_message(ChatMessage {
            id: 1,
            nick: "peer".into(),
            group: String::new(),
            text: format!("message {index}"),
            time: 0,
        });
    }
    for step in 0..12 {
        let pointer = egui::Event::PointerMoved(egui::pos2(500.0, 300.0 + step as f32));
        frame(&context, &mut app, egui::vec2(1178.0, 768.0), vec![pointer]);
        let height = egui::containers::panel::PanelState::load(&context, egui::Id::new("chat")).map(|state| state.rect.height());
        assert_eq!(height, Some(220.0), "chat panel grew on frame {step}");
    }
}

fn recovery_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "recovery"))
                .collect()
        })
        .unwrap_or_default()
}

fn recovering_app(dir: &Path) -> DrawApp {
    let mut app = DrawApp::new();
    app.enable_recovery(Some(dir.to_path_buf()));
    assert!(app.recovery.is_some());
    app
}

fn flush_recovery(app: &DrawApp) {
    app.recovery.as_ref().unwrap().flush();
}

fn text_position(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
        _ => None,
    })
}

#[test]
fn rip_zoom_menu_shortcuts_and_wheel_reach_the_graphical_canvas() {
    use icy_engine_gui::ScalingMode;

    use_english();
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.create(NewKind::Rip, Size::new(80, 25));
    app.settings.monitor_settings.use_integer_scaling = false;
    app.settings.monitor_settings.scaling_mode = ScalingMode::Auto;
    let size = egui::vec2(1280.0, 820.0);
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }

    click_text(&context, &mut app, size, "View");
    click_text(&context, &mut app, size, "Zoom (Fit to Window)");
    click_text(&context, &mut app, size, "2:1  200%");
    frame(&context, &mut app, size, vec![]);
    let editor = app.rip.as_ref().unwrap();
    assert_eq!(editor.zoom(), 2.0);
    assert_eq!(editor.canvas_rect().size(), egui::vec2(1280.0, 700.0));

    frame(&context, &mut app, size, vec![key_event(Key::Minus, egui::Modifiers::COMMAND)]);
    frame(&context, &mut app, size, vec![]);
    assert_eq!(app.rip.as_ref().unwrap().zoom(), 1.5);
    frame(&context, &mut app, size, vec![key_event(Key::Plus, egui::Modifiers::COMMAND)]);
    frame(&context, &mut app, size, vec![]);
    assert_eq!(app.rip.as_ref().unwrap().zoom(), 2.0);

    frame(&context, &mut app, size, vec![key_event(Key::Num0, egui::Modifiers::COMMAND)]);
    frame(&context, &mut app, size, vec![]);
    assert_eq!(app.rip.as_ref().unwrap().canvas_rect().size(), egui::vec2(640.0, 350.0));
    frame(&context, &mut app, size, vec![key_event(Key::Num9, egui::Modifiers::COMMAND)]);
    frame(&context, &mut app, size, vec![]);
    assert_eq!(app.rip.as_ref().unwrap().scaling_mode(), ScalingMode::Auto);

    let editor = app.rip.as_ref().unwrap();
    let before = editor.zoom();
    let pointer = editor.canvas_rect().min + egui::vec2(30.0, 30.0);
    let wheel = || {
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 100.0),
                modifiers: egui::Modifiers::COMMAND,
            },
        ]
    };
    frame(&context, &mut app, size, wheel());
    for _ in 0..20 {
        frame(&context, &mut app, size, vec![]);
    }
    let zoomed = app.rip.as_ref().unwrap().zoom();
    assert!(zoomed > before);
    assert_eq!(app.settings.monitor_settings.scaling_mode, ScalingMode::Manual(zoomed));
    frame(&context, &mut app, size, vec![]);
    assert_eq!(app.rip.as_ref().unwrap().zoom(), zoomed);

    app.dialog = Some(Dialog::New);
    frame(&context, &mut app, size, wheel());
    assert_eq!(app.rip.as_ref().unwrap().zoom(), zoomed);
    app.dialog = None;

    app.settings.monitor_settings.scaling_mode = ScalingMode::Auto;
    frame(&context, &mut app, size, vec![]);
    click_text(&context, &mut app, size, "View");
    let zoom_title = format!("Zoom ({})", menus::zoom_label(app.settings.monitor_settings.scaling_mode));
    click_text(&context, &mut app, size, &zoom_title);
    click_text(&context, &mut app, size, &fl!("menu-zoom-fit_width"));
    frame(&context, &mut app, size, vec![]);
    assert_eq!(app.rip.as_ref().unwrap().scaling_mode(), ScalingMode::FitWidth);
    let fit_width = app.rip.as_ref().unwrap().zoom();
    let short = egui::vec2(1280.0, 400.0);
    for _ in 0..3 {
        frame(&context, &mut app, short, vec![]);
    }
    assert_eq!(app.rip.as_ref().unwrap().zoom(), fit_width, "fit width must allow vertical scrolling");
    app.settings.monitor_settings.scaling_mode = ScalingMode::Auto;
    frame(&context, &mut app, short, vec![]);
    assert!(app.rip.as_ref().unwrap().zoom() < fit_width, "fit window must fit the height too");

    let small = egui::vec2(720.0, 450.0);
    app.settings.monitor_settings.scaling_mode = ScalingMode::Auto;
    for _ in 0..3 {
        frame(&context, &mut app, small, vec![]);
    }
    assert!(app.rip.as_ref().unwrap().zoom() < 0.5, "fit must not force scrolling in a small viewport");
}

#[test]
fn rip_new_open_and_save_use_a_separate_graphical_editor() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.create(NewKind::Rip, Size::new(80, 25));
    assert!(app.rip.is_some());
    let output = frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    assert!(text_position(&output, "Commands").is_some());
    assert!(text_position(&output, "Line").is_some());

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("drawing.rip");
    std::fs::write(&path, b"!|c04|L0A0A1E1E\r\n").unwrap();
    app.open(path.clone());
    assert!(app.rip.is_some());
    assert_eq!(app.document_name(), "drawing.rip");
    assert!(!app.modified());
    let save_as = directory.path().join("copy.rip");
    app.save_path(&context, save_as.clone(), false);
    assert!(app.rip.is_some());
    assert_eq!(
        icy_draw::rip_document::RipDocument::open(&save_as).unwrap().commands(),
        icy_draw::rip_document::RipDocument::open(&path).unwrap().commands()
    );
    assert!(!app.modified());
}

#[test]
fn igs_new_open_and_save_use_their_own_editor() {
    use_english();
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.new_igs_resolution = icy_parser_core::TerminalResolution::Low;
    app.create(NewKind::Igs, Size::new(80, 25));
    assert!(app.igs.is_some() && app.rip.is_none());
    let output = frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    assert!(text_position(&output, "Commands").is_some());
    assert!(text_position(&output, "Line").is_some());
    assert!(text_position(&output, "Low · 320 × 200 · 16 colors").is_some());

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("drawing.ig");
    let bytes = b"G#R>0,2:s>4:\r\nG#C>1,3:L>10,10,100,80:\r\nG#&>0,10,2,0,L,4,x,0,x,50:\r\n";
    std::fs::write(&path, bytes).unwrap();
    app.open(path.clone());
    assert!(app.igs.is_some() && app.rip.is_none());
    assert_eq!(app.document_name(), "drawing.ig");
    assert!(!app.modified());
    frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    let save_as = directory.path().join("copy.ig");
    app.save_path(&context, save_as.clone(), false);
    assert_eq!(std::fs::read(&save_as).unwrap(), bytes);
    assert!(!app.modified());
}

#[test]
fn skypix_new_open_save_and_save_as_use_the_graphical_document() {
    use_english();
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.create(NewKind::Skypix, Size::new(80, 25));
    assert!(app.skypix.is_some());
    assert!(!app.show_start);
    assert!(!app.modified());
    assert_eq!(app.document_name(), fl!("unsaved-title"));

    let directory = tempfile::Builder::new().prefix("skypix-app-").tempdir_in(".").unwrap();
    let path = directory.path().join("drawing.skypix");
    // Retain ordinary text, protocol commands, and an unknown command byte-for-byte.
    let bytes = b"SkyPix\r\n\x1b[9;3! \x1b[1;10;20!\x1b[99;123!\r\n";
    std::fs::write(&path, bytes).unwrap();
    app.open(path.clone());
    assert!(app.skypix.is_some());
    assert_eq!(app.document_name(), "drawing.skypix");
    assert!(!app.modified());
    app.save(&context, false);
    assert!(!app.picker);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    let copy = directory.path().join("copy.skypix");
    app.save_path(&context, copy.clone(), false);
    assert_eq!(std::fs::read(&copy).unwrap(), bytes);
    assert_eq!(app.skypix.as_ref().unwrap().path(), Some(copy.as_path()));
    assert_eq!(app.document_name(), "copy.skypix");
    assert!(!app.modified());

    for extension in FileFormat::SkyPix.all_extensions() {
        let name = format!("legacy.{}", extension.to_uppercase());
        let legacy = directory.path().join(&name);
        std::fs::write(&legacy, bytes).unwrap();
        app.open(legacy);
        assert!(app.skypix.is_some(), "all engine SkyPix extensions must open the graphical editor");
        assert_eq!(app.document_name(), name);
    }
}

#[test]
fn skypix_ansi_fixtures_are_detected_without_renaming_and_save_losslessly() {
    let context = egui::Context::default();
    let directory = tempfile::Builder::new().prefix("skypix-ansi-").tempdir_in(".").unwrap();
    for fixture in ["tests/output/skypix/files/basic/line_test.ans", "tests/output/skypix/files/art/camera.ans"] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../icy_engine").join(fixture);
        let bytes = std::fs::read(&source).unwrap();
        let mut app = DrawApp::new();
        app.open(source.clone());
        assert!(app.skypix.is_some(), "SkyPix commands in {} were not detected", source.display());
        assert!(app.dialog.is_none());
        assert!(!app.modified());
        assert_eq!(app.skypix.as_ref().unwrap().path(), Some(source.as_path()));
        let copy = directory.path().join(source.file_name().unwrap()).with_extension("skypix");
        app.save_path(&context, copy.clone(), false);
        assert_eq!(std::fs::read(copy).unwrap(), bytes);
    }
}

#[test]
fn ordinary_ansi_and_unknown_csi_commands_do_not_open_the_skypix_editor() {
    let directory = tempfile::Builder::new().prefix("ansi-not-skypix-").tempdir_in(".").unwrap();
    for (name, bytes) in [
        ("ordinary.ans", b"\x1b[31mANSI!\x1b[0m\r\n\x1b[!p".as_slice()),
        ("unknown.ans", b"\x1b[777;123!not a known SkyPix command".as_slice()),
        ("invalid-arity.ans", b"\x1b[99;123!malformed EndSkypix command".as_slice()),
        ("unknown-and-soft-reset.ans", b"\x1b[777;123!not a known SkyPix command\x1b[!p".as_slice()),
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let mut app = DrawApp::new();
        app.open(path);
        assert!(app.skypix.is_none(), "{name} should stay in the ANSI editor");
        assert!(app.dialog.is_none());
        assert!(app.document.path.is_some());
    }
}

#[test]
fn skypix_edits_undo_redo_and_close_confirmation_stay_in_the_graphical_editor() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.create(NewKind::Skypix, Size::new(80, 25));
    app.skypix
        .as_mut()
        .unwrap()
        .document
        .append(vec![icy_draw::skypix_document::SkypixItem::command(icy_parser_core::SkypixCommand::SetPixel {
            x: 10,
            y: 20,
        })])
        .unwrap();
    assert!(app.modified());
    assert!(!app.document.modified());
    assert!(app.skypix.as_ref().unwrap().can_undo());
    app.undo(false);
    assert!(!app.modified());
    assert!(app.skypix.as_ref().unwrap().can_redo());
    app.undo(true);
    assert!(app.modified());

    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let output = context.run(input, |context| app.show(context));
    assert!(output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&egui::ViewportCommand::CancelClose));
    assert!(output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&egui::ViewportCommand::Title(format!("*{} — Icy Draw", fl!("unsaved-title")))));
    assert!(matches!(app.dialog, Some(Dialog::Close)));
    app.dialog = None;
    app.request_new();
    assert!(matches!(app.dialog, Some(Dialog::Close)));
    assert!(app.skypix.is_some());
    app.dialog = None;
    let missing = PathBuf::from("missing-skypix-app-file.skypix");
    app.open(missing.clone());
    assert!(matches!(app.dialog, Some(Dialog::Close)));
    assert_eq!(app.pending.as_ref(), Some(&missing));

    app.pending = None;
    app.dialog = None;
    app.quitting = true;
    app.continue_after_save = true;
    let directory = tempfile::Builder::new().prefix("skypix-close-").tempdir_in(".").unwrap();
    let path = directory.path().join("saved.skypix");
    app.save_path(&context, path.clone(), false);
    assert!(path.exists());
    assert!(!app.modified());
    assert!(app.allow_close);
    assert!(!app.continue_after_save);
}

#[test]
fn skypix_replacement_is_mutually_exclusive_with_other_editors() {
    let mut app = DrawApp::new();
    for kind in [
        NewKind::Rip,
        NewKind::Igs,
        NewKind::Animation,
        NewKind::BitmapFont,
        NewKind::TheDraw,
        NewKind::Ansi,
    ] {
        app.create(kind, Size::new(80, 25));
        assert!(app.skypix.is_none());
        app.create(NewKind::Skypix, Size::new(80, 25));
        assert!(app.skypix.is_some());
        assert!(app.rip.is_none() && app.igs.is_none() && app.animation.is_none() && app.font_editor.is_none() && app.charfont.is_none());
        app.create(kind, Size::new(80, 25));
        assert!(app.skypix.is_none());
    }
}

#[test]
fn skypix_rejects_ansi_exports_and_canvas_shortcuts() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.create(NewKind::Skypix, Size::new(80, 25));
    app.canvas_focus = false;
    assert!(!app.command_key(&context, Key::E, egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT)));
    assert!(!app.command_key(&context, Key::G, egui::Modifiers::COMMAND));
    assert!(!app.command_key(&context, Key::E, egui::Modifiers::COMMAND));
    assert!(!app.alt_key(&context, Key::X, egui::Modifiers::ALT));
    assert!(app.dialog.is_none());
    assert!(!app.document.modified());
    let zoom = app.skypix.as_ref().unwrap().zoom();
    app.zoom_step(1);
    assert!(app.skypix.as_ref().unwrap().zoom() > zoom);
    app.command_key(&context, Key::Num0, egui::Modifiers::COMMAND);
    assert_eq!(app.skypix.as_ref().unwrap().zoom(), 1.0);
}

#[test]
fn skypix_failed_open_and_overwrite_preserve_the_active_drawing() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.create(NewKind::Skypix, Size::new(80, 25));
    app.open(PathBuf::from("missing-skypix-app-file.skypix"));
    assert!(matches!(app.dialog, Some(Dialog::Error(_))));
    assert!(app.skypix.is_some());
    app.dialog = None;

    let directory = tempfile::Builder::new().prefix("skypix-overwrite-").tempdir_in(".").unwrap();
    let path = directory.path().join("existing.skypix");
    std::fs::write(&path, b"Keep this drawing").unwrap();
    app.save_path(&context, path.clone(), false);
    assert!(matches!(app.dialog, Some(Dialog::Error(_))));
    assert_eq!(std::fs::read(&path).unwrap(), b"Keep this drawing");
    assert!(app.skypix.as_ref().unwrap().path().is_none());
    app.dialog = None;
    app.save_path(&context, path.clone(), true);
    assert!(app.dialog.is_none());
    assert_eq!(app.skypix.as_ref().unwrap().path(), Some(path.as_path()));
    assert!(!app.modified());
}

#[test]
fn skypix_save_picker_adds_native_extension_and_confirms_overwrite() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    app.create(NewKind::Skypix, Size::new(80, 25));
    let directory = tempfile::Builder::new().prefix("skypix-picker-").tempdir_in(".").unwrap();
    let base = directory.path().join("drawing");
    let path = base.with_extension("skypix");
    app.sender
        .send(Picked {
            action: FileAction::SaveSkypix,
            path: Some(base.clone()),
        })
        .unwrap();
    frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    assert!(path.exists());
    assert!(!base.exists());
    assert_eq!(app.skypix.as_ref().unwrap().path(), Some(path.as_path()));
    app.sender
        .send(Picked {
            action: FileAction::SaveSkypix,
            path: Some(base),
        })
        .unwrap();
    frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
    assert!(matches!(&app.dialog, Some(Dialog::Overwrite(overwrite)) if *overwrite == path));
}

#[test]
fn skypix_autosave_restores_the_graphical_document_and_clears_after_save() {
    let context = egui::Context::default();
    let directory = tempfile::Builder::new().prefix("skypix-recovery-").tempdir_in(".").unwrap();
    let store = directory.path().join("recovery");
    let path = directory.path().join("drawing.skypix");
    let mut app = recovering_app(&store);
    app.create(NewKind::Skypix, Size::new(80, 25));
    app.save_path(&context, path.clone(), false);
    app.skypix
        .as_mut()
        .unwrap()
        .document
        .append(vec![icy_draw::skypix_document::SkypixItem::command(icy_parser_core::SkypixCommand::SetPixel {
            x: 10,
            y: 20,
        })])
        .unwrap();
    app.autosave(&context);
    flush_recovery(&app);
    assert_eq!(recovery_files(&store).len(), 1);
    drop(app);

    let mut restored = recovering_app(&store);
    restored.offer_recovery();
    assert_eq!(restored.offers.len(), 1);
    assert_eq!(
        restored.offers[0].orphan.header.as_ref().unwrap().kind,
        icy_draw::recovery::RecoveryKind::Skypix
    );
    assert_eq!(restored.offers[0].disk, recovery::DiskState::Unchanged);
    restored.restore_offer(0);
    assert!(restored.skypix.is_some());
    assert!(restored.rip.is_none() && restored.igs.is_none());
    assert!(restored.modified());
    assert_eq!(restored.skypix.as_ref().unwrap().path(), Some(path.as_path()));
    restored.save_path(&context, path, false);
    assert!(!restored.modified());
    restored.autosave(&context);
    flush_recovery(&restored);
    assert!(recovery_files(&store).is_empty());
}

#[test]
fn tdf_font_selector_shows_type_icons_without_changing_names_in_both_layouts() {
    use_english();
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let mut font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
    font.state.add_font(icy_engine_edit::charset::TdfFontType::Block, "Blocks".into(), 1);
    font.state.add_font(icy_engine_edit::charset::TdfFontType::Outline, "Outlines".into(), 1);
    app.replace(font.document());
    app.charfont = Some(font);

    for size in [egui::vec2(1280.0, 820.0), egui::vec2(740.0, 820.0)] {
        let selected = "3. Outlines";
        let output = frame(&context, &mut app, size, vec![]);
        let position = text_position(&output, selected).expect("selected font title");
        frame(&context, &mut app, size, vec![egui::Event::PointerMoved(position)]);
        for pressed in [true, false] {
            frame(&context, &mut app, size, pointer(position, pressed));
        }
        let output = frame(&context, &mut app, size, vec![]);
        for label in ["1. New Font", "2. Blocks", selected] {
            assert!(text_position(&output, label).is_some(), "missing font list entry: {label}");
        }
        for icon in ["paint_brush", "rectangle_filled", "rectangle_outline"] {
            assert!(app.icons.loaded(icon), "missing font type icon: {icon}");
        }
        let position = text_position(&output, "1. New Font").unwrap();
        frame(&context, &mut app, size, vec![egui::Event::PointerMoved(position)]);
        for pressed in [true, false] {
            frame(&context, &mut app, size, pointer(position, pressed));
        }
        assert_eq!(app.charfont.as_ref().unwrap().state.selected_font_index(), 0);
        assert!(text_position(&frame(&context, &mut app, size, vec![]), "1. New Font").is_some());
        app.change_charfont(|state| state.select_font(2));
    }
}

#[test]
fn tdf_font_selector_shows_eight_rows_and_scrolls_to_more_fonts() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let mut font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
    font.state.set_font_name("Font1".into());
    for index in 2..=12 {
        font.state.add_font(icy_engine_edit::charset::TdfFontType::Block, format!("Font{index}"), 1);
    }
    font.state.select_font(0);
    app.replace(font.document());
    app.charfont = Some(font);
    let visible = |output: &egui::FullOutput, label: &str| {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.text() == label && shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size())) =>
            {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
    };
    for size in [egui::vec2(1280.0, 820.0), egui::vec2(740.0, 820.0)] {
        app.change_charfont(|state| state.select_font(0));
        frame(&context, &mut app, size, vec![]);
        click_text(&context, &mut app, size, "1. Font1");
        let output = frame(&context, &mut app, size, vec![]);
        let count = (1..=12).filter(|index| visible(&output, &format!("{index}. Font{index}")).is_some()).count();
        assert_eq!(count, 8, "exactly eight fonts fit before scrolling");
        let position = visible(&output, "2. Font2").unwrap();
        frame(
            &context,
            &mut app,
            size,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -300.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..5 {
            frame(&context, &mut app, size, vec![]);
        }
        let output = frame(&context, &mut app, size, vec![]);
        let position = visible(&output, "12. Font12").expect("last font is reachable by scrolling");
        for pressed in [true, false] {
            frame(&context, &mut app, size, pointer(position, pressed));
        }
        assert_eq!(app.charfont.as_ref().unwrap().state.selected_font_index(), 11);
    }
}

fn click_text(context: &egui::Context, app: &mut DrawApp, size: egui::Vec2, label: &str) {
    let output = frame(context, app, size, vec![]);
    let position = text_position(&output, label).unwrap_or_else(|| panic!("no {label:?} on screen"));
    frame(context, app, size, vec![egui::Event::PointerMoved(position)]);
    for pressed in [true, false] {
        frame(context, app, size, pointer(position, pressed));
    }
}

#[test]
fn autosave_restores_an_unsaved_drawing_after_a_crash() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1280.0, 820.0);
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("recovery");
    let art = directory.path().join("art.icy");
    let mut original = Document::new(Size::new(40, 10));
    original.type_text("OLD").unwrap();
    original.save(&art, false).unwrap();

    let mut app = recovering_app(&store);
    app.open(art.clone());
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    assert!(recovery_files(&store).is_empty(), "nothing is unsaved");
    app.document.with_state(|state| state.set_caret_position((0, 1).into()));
    app.document.type_text("NEW").unwrap();
    app.edit(|state| state.add_new_layer(0));
    app.document.with_state(|state| state.set_caret_position((5, 5).into()));
    app.document.type_text("L2").unwrap();
    let output = frame(&context, &mut app, size, vec![]);
    let repaint = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert!(repaint <= recovery::AUTOSAVE_INTERVAL, "idle windows still write later edits ({repaint:?})");
    flush_recovery(&app);
    assert_eq!(recovery_files(&store).len(), 1);

    let mut running = recovering_app(&store);
    running.offer_recovery();
    assert!(running.offers.is_empty(), "documents of running editors are not offered");
    drop(running);
    // A crash: the editor ends without closing its window.
    drop(app);

    let mut restored = recovering_app(&store);
    restored.show_start = true;
    restored.offer_recovery();
    assert_eq!(restored.offers.len(), 1);
    assert_eq!(restored.offers[0].disk, recovery::DiskState::Unchanged);
    frame(&context, &mut restored, size, vec![]);
    assert!(matches!(restored.dialog, Some(Dialog::Recovery)));
    click_text(&context, &mut restored, size, "Restore");
    assert!(restored.dialog.is_none(), "restoring the only document closes the dialog");
    assert!(!restored.show_start);
    assert!(restored.modified());
    assert_eq!(restored.document.path.as_deref(), Some(art.as_path()));
    restored.document.with_state(|state| {
        let buffer = state.get_buffer();
        assert_eq!(buffer.layers.len(), 2);
        let text = |y: i32, from: i32, length: i32| (from..from + length).map(|x| buffer.char_at((x, y).into()).ch).collect::<String>();
        assert_eq!(text(0, 0, 3), "OLD");
        assert_eq!(text(1, 0, 3), "NEW");
        assert_eq!(text(5, 5, 2), "L2");
    });
    flush_recovery(&restored);
    let files = recovery_files(&store);
    assert_eq!(files.len(), 1, "the restored document moved into the new editor's entry");
    assert!(files[0].file_stem().unwrap().to_string_lossy() == restored.recovery.as_ref().unwrap().id());

    restored.save_path(&context, art.clone(), false);
    assert!(restored.dialog.is_none(), "the unchanged original can be saved over");
    frame(&context, &mut restored, size, vec![]);
    flush_recovery(&restored);
    assert!(recovery_files(&store).is_empty(), "saving removes the snapshot");
    let saved = Document::load(&art).unwrap();
    assert_eq!(saved.with_state(|state| state.get_buffer().char_at((0, 1).into()).ch), 'N');
}

#[test]
fn recovered_documents_never_silently_replace_files_changed_since() {
    let context = egui::Context::default();
    let size = egui::vec2(1280.0, 820.0);
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("recovery");
    let art = directory.path().join("art.icy");
    Document::new(Size::new(20, 5)).save(&art, false).unwrap();
    let mut app = recovering_app(&store);
    app.open(art.clone());
    app.document.type_text("MINE").unwrap();
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    drop(app);
    let mut newer = Document::new(Size::new(20, 5));
    newer.type_text("THEIRS").unwrap();
    newer.save(&art, true).unwrap();
    let theirs = std::fs::read(&art).unwrap();

    let mut restored = recovering_app(&store);
    restored.offer_recovery();
    assert_eq!(restored.offers[0].disk, recovery::DiskState::Changed);
    restored.restore_offer(0);
    assert_eq!(restored.document.with_state(|state| state.get_buffer().char_at((0, 0).into()).ch), 'M');
    restored.save_path(&context, art.clone(), false);
    assert!(
        matches!(restored.dialog, Some(Dialog::Error(_))),
        "saving over the newer file needs confirmation"
    );
    assert_eq!(std::fs::read(&art).unwrap(), theirs);
    assert!(restored.modified());
}

#[test]
fn autosave_follows_the_unsaved_state() {
    let context = egui::Context::default();
    let size = egui::vec2(1280.0, 820.0);
    let directory = tempfile::tempdir().unwrap();
    let mut app = recovering_app(directory.path());
    app.document.type_text("A").unwrap();
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    assert_eq!(recovery_files(directory.path()).len(), 1);
    app.document.undo().unwrap();
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    assert!(recovery_files(directory.path()).is_empty(), "undoing all changes leaves nothing to recover");

    // Undoing past the save point and then editing must not look unmodified.
    let art = directory.path().join("art.icy");
    app.document.type_text("B").unwrap();
    app.save_path(&context, art.clone(), false);
    app.document.undo().unwrap();
    app.document.type_text("C").unwrap();
    assert!(app.modified());
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    assert_eq!(recovery_files(directory.path()).len(), 1);

    // Later edits replace the snapshot once the interval has passed.
    app.document.type_text("D").unwrap();
    app.next_autosave = Some(std::time::Instant::now());
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    let snapshot = icy_draw::recovery::orphans(directory.path(), None);
    assert!(snapshot.is_empty(), "the running editor keeps its entry locked");
    let bytes = std::fs::read(&recovery_files(directory.path())[0]).unwrap();
    let id = app.recovery.as_ref().unwrap().id().to_owned();
    drop(app);
    let orphan = icy_draw::recovery::claim(directory.path(), &id).unwrap();
    let recovered = Document::from_recovery(&orphan.load().unwrap()).unwrap();
    assert_eq!(recovered.with_state(|state| state.get_buffer().char_at((1, 0).into()).ch), 'D');
    assert!(!bytes.is_empty());
}

#[test]
fn discarding_changes_on_quit_removes_the_snapshot() {
    let context = egui::Context::default();
    let size = egui::vec2(1280.0, 820.0);
    let directory = tempfile::tempdir().unwrap();
    let mut app = recovering_app(directory.path());
    app.document.type_text("GONE").unwrap();
    frame(&context, &mut app, size, vec![]);
    app.quitting = true;
    app.complete_close(&context);
    assert!(recovery_files(directory.path()).is_empty(), "removed before the window closes");
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    assert!(recovery_files(directory.path()).is_empty(), "not written again while closing");
}

#[test]
fn recovery_dialog_can_postpone_and_discard() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1280.0, 820.0);
    let directory = tempfile::tempdir().unwrap();
    let mut app = recovering_app(directory.path());
    app.document.type_text("LATER").unwrap();
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    drop(app);

    let mut first = recovering_app(directory.path());
    first.offer_recovery();
    frame(&context, &mut first, size, vec![]);
    click_text(&context, &mut first, size, "Decide Later");
    assert!(first.dialog.is_none() && first.offers.is_empty());
    assert_eq!(recovery_files(directory.path()).len(), 1, "postponing keeps the file");
    drop(first);

    let mut second = recovering_app(directory.path());
    second.offer_recovery();
    assert_eq!(second.offers.len(), 1, "postponed documents are offered again");
    frame(&context, &mut second, size, vec![]);
    click_text(&context, &mut second, size, "Discard…");
    assert_eq!(recovery_files(directory.path()).len(), 1, "discarding asks first");
    click_text(&context, &mut second, size, "Discard Permanently");
    assert!(recovery_files(directory.path()).is_empty());
    assert!(second.offers.is_empty());
    frame(&context, &mut second, size, vec![]);
    assert!(second.dialog.is_none());
}

#[test]
fn new_document_groups_cover_all_editors_in_the_same_order() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1280.0, 820.0);
    let mut app = DrawApp::new();
    app.show_start = true;
    let groups = NewKind::groups();
    assert!(groups[0].1 == [NewKind::Ansi, NewKind::Animation, NewKind::Rip]);
    assert!(groups[1].1 == [NewKind::Atascii, NewKind::Vt52, NewKind::Igs, NewKind::Petscii, NewKind::Skypix]);
    assert!(groups[2].1 == [NewKind::BitmapFont, NewKind::TheDraw]);
    let welcome = frame(&context, &mut app, size, vec![]);
    for (title, kinds) in &groups {
        let header = text_position(&welcome, &title.to_uppercase()).unwrap();
        for kind in *kinds {
            assert!(text_position(&welcome, &kind.name()).unwrap().y > header.y);
        }
    }
    click_text(&context, &mut app, size, "Custom…");
    assert!(matches!(app.dialog, Some(Dialog::New)));
    app.show_start = false;
    for _ in 0..3 {
        frame(&context, &mut app, size, vec![]);
    }
    let dialog = frame(&context, &mut app, size, vec![]);
    for (title, kinds) in &groups {
        let header = text_position(&dialog, title).unwrap();
        for kind in *kinds {
            assert!(text_position(&dialog, &kind.name()).unwrap().y > header.y);
            assert!(
                dialog.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == kind.name() => {
                        shape.clip_rect.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => false,
                }),
                "all editor types fit in the dialog at desktop size"
            );
        }
    }
}

#[test]
fn tdf_welcome_tile_opens_the_new_dialog_with_a_type_selector() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1280.0, 820.0);
    for (label, kind) in [
        ("Color", icy_engine_edit::charset::TdfFontType::Color),
        ("Block", icy_engine_edit::charset::TdfFontType::Block),
        ("Outline", icy_engine_edit::charset::TdfFontType::Outline),
    ] {
        let mut app = DrawApp::new();
        app.show_start = true;
        assert_eq!(
            NewKind::groups()
                .iter()
                .flat_map(|(_, kinds)| kinds.iter())
                .filter(|&&kind| kind == NewKind::TheDraw)
                .count(),
            1
        );
        click_text(&context, &mut app, size, "TDF Font");
        assert!(matches!(app.dialog, Some(Dialog::New)));
        assert!(app.new_kind == NewKind::TheDraw);
        assert!(app.charfont.is_none(), "the tile must not create a font immediately");
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        if label != "Color" {
            click_text(&context, &mut app, size, "Color");
            click_text(&context, &mut app, size, label);
        }
        click_text(&context, &mut app, size, "Create");
        assert!(app.dialog.is_none(), "Create did not close the {label} dialog");
        assert_eq!(app.charfont.as_ref().unwrap().state.selected_font().unwrap().font_type(), kind);
    }
}

#[test]
#[ignore]
fn gpu_new_tdf_dialog_renders() {
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.show_start = true;
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "new-groups-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "new-groups");
        app.new_kind = NewKind::TheDraw;
        app.dialog = Some(Dialog::New);
        for _ in 0..4 {
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], "new-tdf-warmup");
        }
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "new-tdf");
    });
}

#[test]
#[ignore]
fn gpu_tdf_font_dropdown_renders_eight_icon_rows() {
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        let mut font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
        for index in 2..=12 {
            let kind = match index % 3 {
                0 => icy_engine_edit::charset::TdfFontType::Outline,
                1 => icy_engine_edit::charset::TdfFontType::Color,
                _ => icy_engine_edit::charset::TdfFontType::Block,
            };
            font.state.add_font(kind, format!("Font {index}"), 1);
        }
        font.state.select_font(0);
        app.replace(font.document());
        app.charfont = Some(font);
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tdf-font-dropdown-warmup");
        let output = frame(&gpu.context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
        let position = text_position(&output, "1. New Font").unwrap();
        for pressed in [true, false] {
            gpu.capture(&mut app, [1280, 820], 1.0, pointer(position, pressed), "tdf-font-dropdown-warmup");
        }
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "tdf-font-dropdown");
    });
}

#[test]
fn autosave_restores_fonts_and_animations() {
    let context = egui::Context::default();
    let size = egui::vec2(1280.0, 820.0);
    let directory = tempfile::tempdir().unwrap();
    let restore = |check: &dyn Fn(&DrawApp)| {
        let mut restored = recovering_app(directory.path());
        restored.offer_recovery();
        assert_eq!(restored.offers.len(), 1);
        restored.restore_offer(0);
        assert!(restored.dialog.is_none());
        assert!(restored.modified());
        check(&restored);
        restored.quitting = true;
        restored.complete_close(&context);
    };

    let mut app = recovering_app(directory.path());
    app.create(NewKind::TheDraw, Size::new(80, 25));
    app.document.type_text("X").unwrap();
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    drop(app);
    restore(&|app| {
        let font = app.charfont.as_ref().expect("TheDraw editor");
        assert!(font.state.get_glyph('A').is_some(), "the glyph being edited is included");
        assert_eq!(app.document.with_state(|state| state.get_buffer().char_at((0, 0).into()).ch), 'X');
    });

    let mut app = recovering_app(directory.path());
    app.create(NewKind::BitmapFont, Size::new(80, 25));
    let editor = app.font_editor.as_mut().unwrap();
    let before = editor.state.get_glyph_pixels('A')[0][0];
    editor.state.set_pixel('A', 0, 0, !before).unwrap();
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    drop(app);
    restore(&|app| {
        let editor = app.font_editor.as_ref().expect("bitmap font editor");
        assert_eq!(editor.state.get_glyph_pixels('A')[0][0], !before);
    });

    let mut app = recovering_app(directory.path());
    app.create(NewKind::Animation, Size::new(80, 25));
    app.animation.as_mut().unwrap().replace_text(0, 0, "-- unsaved\n").unwrap();
    frame(&context, &mut app, size, vec![]);
    flush_recovery(&app);
    drop(app);
    restore(&|app| assert_eq!(app.animation.as_ref().expect("animation editor").source, "-- unsaved\n"));
    assert!(recovery_files(directory.path()).is_empty());
}

#[test]
fn shading_steps_through_custom_character_and_color_ramps() {
    use icy_draw::brush::{char_ramp_from_text, Ramp};
    use icy_engine::MouseButton;
    let mut app = DrawApp::new();
    app.document.tool = Tool::Pencil;
    app.document.brush.primary = BrushPrimaryMode::Shading;
    app.document.brush.shade_chars = char_ramp_from_text(".oO").unwrap();
    app.document.brush.shade_colors = Ramp::new(&[1, 9, 11]);
    let cell = |app: &DrawApp| {
        app.document.with_state(|state| {
            let cell = state.get_buffer().char_at((2, 1).into());
            (cell.ch, cell.attribute.foreground())
        })
    };
    let stroke = |app: &mut DrawApp, button| {
        app.document.begin(Position::new(2, 1), button);
        app.document.finish();
    };
    for expected in [('.', 1), ('o', 9), ('O', 11), ('O', 11)] {
        stroke(&mut app, MouseButton::Left);
        assert_eq!(cell(&app), expected);
    }
    stroke(&mut app, MouseButton::Right);
    assert_eq!(cell(&app), ('o', 9), "right-click lightens both ramps");

    // Without the foreground filter the color ramp is not used.
    app.document.brush.colorize_fg = false;
    stroke(&mut app, MouseButton::Left);
    assert_eq!(cell(&app), ('O', 9));

    // Keeping the characters only recolors.
    app.document.brush.colorize_fg = true;
    app.document.brush.shade_chars = Default::default();
    app.document.brush.shade_colors = Ramp::new(&[8, 7, 15]);
    for expected in [('O', 8), ('O', 7), ('O', 15)] {
        stroke(&mut app, MouseButton::Left);
        assert_eq!(cell(&app), expected);
    }
    app.document.undo().unwrap();
    assert_eq!(cell(&app), ('O', 7), "each stroke is one undo step");
}

#[test]
fn shade_toolbar_picks_ramps_and_the_editor_updates_the_brush() {
    use icy_draw::brush::{char_ramp_from_text, Ramp, ShadeRamps};
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1280.0, 820.0);
    let mut app = DrawApp::new();
    app.select_tool(Tool::Pencil);
    app.document.brush.primary = BrushPrimaryMode::Shading;
    frame(&context, &mut app, size, vec![]);
    let output = frame(&context, &mut app, size, vec![]);
    for label in ["Brush color"] {
        assert!(text_position(&output, label).is_some(), "missing {label:?} in the shading toolbar");
    }
    click_text(&context, &mut app, size, "Brush color");
    click_text(&context, &mut app, size, "Edit Ramps…");
    assert!(matches!(app.dialog, Some(Dialog::ShadeRamps(_))));

    // Editing the ramp the brush uses switches the brush to the edited ramp.
    let mut ramps = ShadeRamps::default();
    ramps.characters[0] = ".:#".into();
    ramps.colors.insert(0, vec![3, 11]);
    app.apply_shade_ramps(ramps.clone());
    assert_eq!(app.document.brush.shade_chars, char_ramp_from_text(".:#").unwrap());
    assert!(app.document.brush.shade_colors.is_empty(), "the brush color is not a ramp of the list");
    app.document.brush.shade_colors = Ramp::new(&[3, 11]);
    ramps.colors[0] = vec![3, 11, 15];
    app.apply_shade_ramps(ramps.clone());
    assert_eq!(app.document.brush.shade_colors, Ramp::new(&[3, 11, 15]));
    assert_eq!(app.settings.shade_ramps, ramps);
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_shade_ramp_controls_render() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.select_tool(Tool::Pencil);
        app.document.brush.primary = BrushPrimaryMode::Shading;
        app.document.brush.shade_colors = icy_draw::brush::Ramp::new(&[1, 9, 11, 15]);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            gpu.context.set_theme(theme);
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], "shade-toolbar-warmup");
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], &format!("shade-toolbar-{theme:?}"));
        }
        gpu.context.set_theme(egui::Theme::Dark);
        app.open_shade_ramps();
        if let Some(Dialog::ShadeRamps(draft)) = &mut app.dialog {
            draft.ramps.characters.push("░€".into());
        }
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "shade-ramps-warmup");
        let pixels = gpu.capture(&mut app, [1280, 820], 1.0, vec![], "shade-ramps");
        assert!(
            pixels.chunks(4).any(|pixel| pixel[0] > 150 && pixel[1] < 90),
            "the invalid ramp is reported in red"
        );
    });
}

#[test]
fn gpu_chip_tune_editor_renders() {
    use icy_draw::igs_tune::{NoteEnd, Tune, TuneNote};
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.create(NewKind::Igs, Size::new(80, 25));
        let note = |start, length, voice, pitch, end| TuneNote {
            start,
            length,
            voice,
            pitch,
            effect: icy_parser_core::SoundEffect::Longbell,
            volume: 15,
            end,
        };
        let tune = Tune {
            notes: vec![
                note(0, 40, 0, 72, NoteEnd::Release),
                note(40, 40, 0, 74, NoteEnd::Release),
                note(80, 80, 0, 76, NoteEnd::Cut),
                note(0, 160, 1, 60, NoteEnd::Hold),
                note(160, 60, 2, 67, NoteEnd::Release),
            ],
        };
        app.igs.as_mut().unwrap().open_tune_for_test(tune);
        for _ in 0..3 {
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], "chip-tune-warmup");
        }
        let pixels = gpu.capture(&mut app, [1280, 820], 1.0, vec![], "chip-tune");
        for color in crate::igs::tune::VOICE_COLORS {
            assert!(
                pixels.chunks(4).any(|pixel| pixel[..3] == [color.r(), color.g(), color.b()]),
                "the notes of every voice are drawn in its color"
            );
        }
    });
}

#[test]
fn gpu_command_editors_show_the_player_under_the_canvas() {
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let directory = tempfile::tempdir().unwrap();
        let rip = directory.path().join("drawing.rip");
        std::fs::write(&rip, b"!|c04|L0A0A1E1E\r\n").unwrap();
        for kind in [NewKind::Igs, NewKind::Rip] {
            let mut app = DrawApp::new();
            if kind == NewKind::Rip {
                app.open(rip.clone());
            } else {
                app.create(kind, Size::new(80, 25));
            }
            let name = format!("player-{}", if kind == NewKind::Igs { "igs" } else { "rip" });
            for _ in 0..3 {
                gpu.capture(&mut app, [1280, 820], 1.0, vec![], &format!("{name}-warmup"));
            }
            let pixels = gpu.capture(&mut app, [1280, 820], 1.0, vec![], &name);
            let green = pixels
                .chunks(4)
                .enumerate()
                .filter(|(_, pixel)| pixel[..3] == [widgets::PLAY.r(), widgets::PLAY.g(), widgets::PLAY.b()])
                .map(|(index, _)| index / 1280)
                .collect::<Vec<_>>();
            assert!(!green.is_empty(), "{name}: the round play button is drawn");
            assert!(green.iter().all(|&row| row > 620), "{name}: the player is under the canvas, not at the top");
        }
    });
}

#[test]
fn gpu_igs_editor_with_a_drawing_renders() {
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../icy_parser_core/benches/igs_data/KM-1FED.IG"));
        for _ in 0..3 {
            gpu.capture(&mut app, [1280, 820], 1.0, vec![], "igs-drawing-warmup");
        }
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "igs-drawing");
    });
}

#[test]
fn start_page_connects_to_a_server() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let size = egui::vec2(1280.0, 820.0);
    let mut app = DrawApp::new();
    app.show_start = true;
    frame(&context, &mut app, size, vec![]);
    click_text(&context, &mut app, size, "Connect to Server…");
    assert!(matches!(app.dialog, Some(Dialog::Connect)));
}

#[test]
fn shared_dialog_labels_are_translated() {
    // The shared crate's translations are embedded separately; if they were missing, every
    // shared dialog button would read "No localization for id".
    for label in [
        icy_engine_gui::egui::appearance::labels::ok(),
        icy_engine_gui::egui::appearance::labels::cancel(),
        icy_engine_gui::egui::appearance::labels::close(),
    ] {
        assert!(!label.contains("No localization"), "{label}");
    }
}

#[test]
fn escape_without_a_selection_opens_the_attribute_picker() {
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    frame(&context, &mut app, size, vec![]);
    let caret = |app: &DrawApp| {
        app.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (attribute.foreground(), attribute.background())
        })
    };
    assert_eq!(caret(&app), (7, 0));

    // With a selection Escape only deselects.
    app.select_all();
    frame(&context, &mut app, size, vec![key_event(Key::Escape, egui::Modifiers::NONE)]);
    assert!(!app.document.with_state(|state| state.is_something_selected()));
    assert!(app.attribute_picker.is_none());

    frame(&context, &mut app, size, vec![key_event(Key::Escape, egui::Modifiers::NONE)]);
    assert!(app.attribute_picker.is_some(), "the next Escape opens the picker");
    frame(&context, &mut app, size, vec![]);
    assert!(app.attribute_picker.is_some(), "the Escape that opened it does not close it");

    // Up and down change the foreground, left and right the background, at once and wrapping.
    frame(&context, &mut app, size, vec![key_event(Key::ArrowDown, egui::Modifiers::NONE)]);
    frame(&context, &mut app, size, vec![key_event(Key::ArrowLeft, egui::Modifiers::NONE)]);
    assert_eq!(caret(&app), (8, 15), "the default palette has iCE colors");
    frame(&context, &mut app, size, vec![key_event(Key::Enter, egui::Modifiers::NONE)]);
    assert!(app.attribute_picker.is_none());
    assert_eq!(caret(&app), (8, 15), "closing keeps the colors");
    frame(&context, &mut app, size, vec![egui::Event::Text("A".into())]);
    assert_eq!(
        app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch),
        'A',
        "typing works again"
    );
}

#[test]
fn keys_reach_the_canvas_after_clicking_panels_and_follow_moebius() {
    let context = egui::Context::default();
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.document.tool = Tool::Click;
    for _ in 0..2 {
        frame(&context, &mut app, size, vec![]);
    }
    // A click beside the canvas used to leave the keyboard nowhere, so F1 typed nothing.
    let rail = egui::pos2(app.canvas_rect.right() + 150.0, app.canvas_rect.bottom() - 4.0);
    for pressed in [true, false] {
        frame(&context, &mut app, size, pointer(rail, pressed));
    }
    frame(&context, &mut app, size, vec![key_event(Key::F1, egui::Modifiers::NONE)]);
    let expected = app.settings.fkeys.code_at(app.settings.fkeys.current_set(), 0) as u32;
    assert_eq!(
        app.document.with_state(|state| state.get_buffer().char_at(Position::default()).ch as u32),
        expected
    );

    let colors = |app: &DrawApp| {
        app.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            (attribute.foreground(), attribute.background())
        })
    };
    frame(&context, &mut app, size, vec![key_event(Key::Num4, egui::Modifiers::COMMAND)]);
    assert_eq!(colors(&app).0, 4, "Ctrl+4 picks dark red");
    frame(&context, &mut app, size, vec![key_event(Key::Num4, egui::Modifiers::COMMAND)]);
    assert_eq!(colors(&app).0, 12, "pressed again it turns bright");
    frame(&context, &mut app, size, vec![key_event(Key::Num1, egui::Modifiers::ALT)]);
    assert_eq!(colors(&app).1, 1, "Alt+1 picks the background");
    frame(&context, &mut app, size, vec![key_event(Key::D, egui::Modifiers::COMMAND)]);
    assert_eq!(colors(&app), (7, 0), "Ctrl+D restores the default colors");

    // Zoom keys follow the desktop convention of Icy Term and Icy View.
    frame(&context, &mut app, size, vec![key_event(Key::Num9, egui::Modifiers::COMMAND)]);
    assert_eq!(app.settings.monitor_settings.scaling_mode, ScalingMode::Auto);
    frame(&context, &mut app, size, vec![key_event(Key::Num0, egui::Modifiers::COMMAND)]);
    assert_eq!(app.settings.monitor_settings.scaling_mode, ScalingMode::Manual(1.0));

    let ice = |app: &DrawApp| app.document.with_state(|state| state.get_buffer().ice_mode);
    let before = ice(&app);
    frame(&context, &mut app, size, vec![key_event(Key::E, egui::Modifiers::COMMAND)]);
    assert_ne!(ice(&app), before, "Ctrl+E toggles iCE colors");
    assert!(app.dialog.is_none(), "export moved to Ctrl+Shift+E");

    // Outside the text tools K switches to the keyboard tool without typing the letter.
    app.document.tool = Tool::Pencil;
    let before = app.document.with_state(|state| state.get_buffer().char_at(Position::new(1, 0)).ch);
    frame(
        &context,
        &mut app,
        size,
        vec![key_event(Key::K, egui::Modifiers::NONE), egui::Event::Text("k".into())],
    );
    assert_eq!(app.document.tool, Tool::Click);
    assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(1, 0)).ch), before);
    app.document.tool = Tool::Select;
    frame(&context, &mut app, size, vec![key_event(Key::I, egui::Modifiers::NONE)]);
    assert_eq!((app.document.tool, app.document.brush.primary), (Tool::Pencil, BrushPrimaryMode::Shading));
    frame(&context, &mut app, size, vec![key_event(Key::P, egui::Modifiers::NONE)]);
    assert_eq!(app.document.tool, Tool::Fill);
}

#[test]
fn line_tool_outline_mode_shows_its_styles_like_shading() {
    use icy_draw::box_lines::BoxStyle;
    use_english();
    let context = egui::Context::default();
    appearance::apply(&context);
    let mut app = DrawApp::new();
    let size = egui::vec2(1280.0, 820.0);
    app.document.tool = Tool::Line;
    let output = frame(&context, &mut app, size, vec![]);
    assert!(text_position(&output, "Outline").is_some(), "outline is one of the line tool's modes");
    assert!(text_position(&output, "\u{256c}").is_none(), "the styles only show in outline mode");
    click_text(&context, &mut app, size, "Outline");
    assert_eq!(app.document.box_line, Some(BoxStyle::Single));
    click_text(&context, &mut app, size, "\u{256c}");
    assert_eq!(app.document.box_line, Some(BoxStyle::Double));
    click_text(&context, &mut app, size, "Character");
    assert_eq!(app.document.box_line, None);
    assert_eq!(app.document.brush.primary, BrushPrimaryMode::Char);
    let output = frame(&context, &mut app, size, vec![]);
    assert!(text_position(&output, "\u{256c}").is_none());
    click_text(&context, &mut app, size, "Outline");
    assert_eq!(app.document.box_line, Some(BoxStyle::Double), "the last style is kept");

    app.document.tool = Tool::RectangleOutline;
    let output = frame(&context, &mut app, size, vec![]);
    assert!(text_position(&output, "Outline").is_some(), "rectangle outlines draw box frames too");
    app.document.tool = Tool::RectangleFilled;
    let output = frame(&context, &mut app, size, vec![]);
    assert!(text_position(&output, "Outline").is_none(), "filled rectangles have no outline mode");
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_box_lines_render() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.document.tool = Tool::Line;
        app.document.brush.primary = BrushPrimaryMode::Char;
        let mut line = |style, start: (i32, i32), end: (i32, i32)| {
            app.document.box_line = Some(style);
            app.document.box_style = style;
            app.document.begin(Position::new(start.0, start.1), icy_engine::MouseButton::Left);
            app.document.update(Position::new(end.0, end.1));
            app.document.finish();
        };
        use icy_draw::box_lines::BoxStyle;
        // A double frame with a single divider and a single column that crosses it.
        line(BoxStyle::Double, (2, 1), (30, 1));
        line(BoxStyle::Double, (30, 1), (30, 10));
        line(BoxStyle::Double, (30, 10), (2, 10));
        line(BoxStyle::Double, (2, 10), (2, 1));
        line(BoxStyle::Single, (2, 4), (30, 4));
        line(BoxStyle::Single, (14, 1), (14, 10));
        // An elbow connector from the frame.
        line(BoxStyle::Single, (30, 7), (40, 12));
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "box-lines-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "box-lines");
        // A line being dragged across the frame previews its characters and joins.
        app.document.box_line = Some(BoxStyle::Double);
        app.document.box_style = BoxStyle::Double;
        let info = app.view.terminal.render_info.read().clone();
        let cell = |x: f32, y: f32| {
            egui::pos2(
                info.bounds_x + info.viewport_x + info.font_width * info.display_scale * (x + 0.5),
                info.bounds_y + info.viewport_y + info.font_height * info.display_scale * (y + 0.5),
            )
        };
        let (start, end) = (cell(8.0, 7.0), cell(36.0, 7.0));
        let press = |pressed| egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        gpu.capture(
            &mut app,
            [1280, 820],
            1.0,
            vec![egui::Event::PointerMoved(start), press(true)],
            "box-lines-drag-warmup",
        );
        gpu.capture(&mut app, [1280, 820], 1.0, vec![egui::Event::PointerMoved(end)], "box-lines-drag-warmup");
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "box-lines-drag");
        assert!(!app.document.preview_cells.is_empty(), "the drag is still running");
    });
}

#[test]
fn gpu_atascii_pixels_follow_the_pointer() {
    use_english();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut gpu = Gpu::new().await;
        let mut app = DrawApp::new();
        app.create(NewKind::Atascii, Size::new(80, 25));
        app.document.tool = Tool::Pencil;
        app.document.quarter_blocks = true;
        gpu.capture(&mut app, [1280, 820], 1.0, vec![], "atascii-pixels-warmup");
        let info = app.view.terminal.render_info.read().clone();
        // The centre of pixel (x, y), two per character in each direction.
        let pixel = |x: f32, y: f32| {
            egui::pos2(
                info.bounds_x + info.viewport_x + info.font_width * info.display_scale * (x + 0.5) / 2.0,
                info.bounds_y + info.viewport_y + info.font_height * info.display_scale * (y + 0.5) / 2.0,
            )
        };
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        // A click on the lower right pixel of character (3, 2).
        let target = pixel(7.0, 5.0);
        gpu.capture(
            &mut app,
            [1280, 820],
            1.0,
            vec![egui::Event::PointerMoved(target), press(target, true)],
            "atascii-pixels-warmup",
        );
        gpu.capture(&mut app, [1280, 820], 1.0, vec![press(target, false)], "atascii-pixels");
        let code = app.document.with_state(|state| state.get_buffer().char_at(Position::new(3, 2)).ch as u32);
        assert_eq!(code, 0x09, "the lower right quarter block ▗");
    });
}
