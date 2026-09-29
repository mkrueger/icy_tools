use super::*;
use icy_engine_gui::ScalingMode;
use icy_mail::reader::{MessageColumn, NavigateDirection, Pane, ViewMode};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// A window with the test packet open; drafts, read marks and the recent list stay in the packet's directory.
fn loaded(context: &egui::Context) -> (packet_tests::TempDir, app::MailApp) {
    let (dir, _package) = packet_tests::load();
    let mut mail = app::MailApp::with_storage(context, dir.path().to_path_buf());
    mail.open(dir.path().join("TEST.QWK"), context);
    wait(&mut mail, context);
    (dir, mail)
}

fn settle(context: &egui::Context, mail: &mut app::MailApp, size: egui::Vec2) -> egui::FullOutput {
    for _ in 0..3 {
        frame(context, mail, size, vec![]);
    }
    frame(context, mail, size, vec![])
}

fn count(output: &egui::FullOutput, label: &str) -> usize {
    output
        .shapes
        .iter()
        .filter(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label))
        .count()
}

fn click_label(context: &egui::Context, mail: &mut app::MailApp, size: egui::Vec2, name: &str) {
    let output = frame(context, mail, size, vec![]);
    let position = label(&output, name).center();
    for pressed in [true, false] {
        frame(context, mail, size, pointer(position, pressed));
    }
}

#[test]
fn about_artwork_shows_version_and_build_date() {
    use icy_engine::TextPane;
    let build_date = option_env!("ICY_BUILD_DATE").expect("build.rs sets ICY_BUILD_DATE");
    let mut screen = icy_engine::FileFormat::IcyDraw
        .from_bytes(include_bytes!("../../../data/about.icy"), None)
        .unwrap()
        .screen;
    icy_engine_gui::version_helper::replace_version_marker(&mut screen.buffer, &icy_mail::VERSION, Some(build_date.to_string()));
    let buffer = &screen.buffer;
    let text: String = (0..buffer.height())
        .flat_map(|y| (0..buffer.width()).map(move |x| (x, y)))
        .map(|(x, y)| buffer.char_at((x, y).into()).ch)
        .collect();
    assert!(text.contains(&format!("v{}", *icy_mail::VERSION)));
    assert!(text.contains(&build_date[..10]), "build date missing from the about artwork");
    assert!(!text.contains('@'));
}

#[test]
fn about_dialog_shows_the_shared_artwork_dialog() {
    let context = egui::Context::default();
    icy_engine_gui::egui::appearance::apply(&context);
    let mut mail = app::MailApp::new(&context);
    mail.open_about();
    assert!(matches!(mail.modal, Some(app::Modal::About)));
    let output = settle(&context, &mut mail, egui::vec2(1100.0, 760.0));
    let close = icy_engine_gui::egui::appearance::labels::close();
    assert!(mail.about.is_some());
    assert!(output
        .shapes
        .iter()
        .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == close)));
}

#[test]
fn available_update_is_shown_in_the_status_bar() {
    crate::use_english();
    let context = egui::Context::default();
    let mut mail = app::MailApp::new(&context);
    mail.latest_version = Some(semver::Version::new(9, 8, 7));
    let output = settle(&context, &mut mail, egui::vec2(1100.0, 760.0));
    label(&output, "Update available: 9.8.7");
}

/// Texts painted with a highlighted (search match) background.
fn highlighted(output: &egui::FullOutput) -> Vec<String> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(
                text.galley
                    .job
                    .sections
                    .iter()
                    .filter(|section| section.format.background != egui::Color32::TRANSPARENT)
                    .map(|section| text.galley.job.text[section.byte_range.clone()].to_string())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

fn search_messages(context: &egui::Context, mail: &mut app::MailApp, query: &str) {
    let size = egui::vec2(1100.0, 760.0);
    mail.reader.filter = query.into();
    mail.filter_changed();
    frame(context, mail, size, vec![]);
    let deadline = Instant::now() + Duration::from_secs(5);
    while mail.loader.searching {
        assert!(Instant::now() < deadline, "body search timed out");
        std::thread::yield_now();
        frame(context, mail, size, vec![]);
    }
}

#[test]
fn body_highlights_preserve_colors_selection_and_copied_text() {
    use icy_engine::{AttributeColor, Selection};
    use icy_engine_gui::egui::screen::ScreenView;

    let mut view = ScreenView::new(icy_mail::reader::render_body(b"\x1b[31mGr\x81\xE1e GR\x9a\xE1E\x1b[0m\nGr\x81\xE1e").unwrap());
    let mut highlights = reader_view::BodyHighlights::default();
    let positions: Vec<_> = (0..2).flat_map(|row| (0..12).map(move |column| (column, row).into())).collect();
    let original: Vec<_> = positions.iter().map(|&position| view.terminal.screen.lock().char_at(position)).collect();
    let mut selection = Selection::new((0, 0));
    selection.lead = (4, 0).into();
    view.terminal.screen.lock().set_selection(selection).unwrap();
    let copied = view.terminal.screen.lock().copy_text();
    for dark_mode in [true, false] {
        highlights.update(&mut view, " GRÜßE ", dark_mode).unwrap();
        let screen = view.terminal.screen.lock();
        let color = widgets::search_highlight_color(dark_mode);
        for (column, row) in [(0, 0), (4, 0), (6, 0), (10, 0), (0, 1), (4, 1)] {
            let ch = screen.char_at((column, row).into());
            assert_eq!(ch.attribute.background_color(), AttributeColor::Rgb(color.r(), color.g(), color.b()));
            assert_eq!(ch.attribute.foreground_color(), AttributeColor::Rgb(0, 0, 0));
        }
        assert_eq!(screen.char_at((5, 0).into()), original[5], "spaces outside matches keep their ANSI colors");
        assert_eq!(screen.selection(), Some(selection));
        assert_eq!(screen.copy_text(), copied);
    }
    for query in ["missing", ""] {
        highlights.update(&mut view, query, true).unwrap();
        let restored: Vec<_> = positions.iter().map(|&position| view.terminal.screen.lock().char_at(position)).collect();
        assert_eq!(restored, original);
    }
    highlights.update(&mut view, "grüße", true).unwrap();
    view = ScreenView::new(icy_mail::reader::render_body(b"\x1b[32mNew message").unwrap());
    let first = view.terminal.screen.lock().char_at((0, 0).into());
    highlights.update(&mut view, "grüße", true).unwrap();
    assert_eq!(
        view.terminal.screen.lock().char_at((0, 0).into()),
        first,
        "old colors must not leak into another message"
    );
}

#[test]
fn message_search_highlights_body_and_clears_with_filter() {
    use icy_engine::AttributeColor;

    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    // The classic terminal view is what this test is about.
    mail.reading_mode = icy_mail::options::ReadingMode::Classic;
    search_messages(&context, &mut mail, "LINE 1");
    wait(&mut mail, &context);
    settle(&context, &mut mail, egui::vec2(1100.0, 760.0));
    {
        let screen = mail.screen.terminal.screen.lock();
        let color = widgets::search_highlight_color(context.style().visuals.dark_mode);
        for column in 0..6 {
            assert_eq!(
                screen.char_at((column, 1).into()).attribute.background_color(),
                AttributeColor::Rgb(color.r(), color.g(), color.b())
            );
        }
        assert_eq!(screen.char_at((0, 0).into()).attribute.background_color(), AttributeColor::Palette(0));
    }
    search_messages(&context, &mut mail, "");
    assert_eq!(
        mail.screen.terminal.screen.lock().char_at((0, 1).into()).attribute.background_color(),
        AttributeColor::Palette(0)
    );
}

#[test]
fn message_search_finds_unopened_bodies_and_preserves_filters() {
    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    search_messages(&context, &mut mail, "LiNe 4");
    assert!(mail.error.is_none(), "{:?}", mail.error);
    assert_eq!(mail.reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [2]);
    search_messages(&context, &mut mail, "line 1");
    assert_eq!(mail.reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [0, 2, 3]);
    assert!(!mail.reader.is_read(3), "searching a body must not mark it as read");
    mail.select_folder(app::Folder::Conference(1));
    assert_eq!(mail.reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [0]);
    mail.set_mode(ViewMode::Threads);
    assert_eq!(mail.reader.messages.len(), 1);
    mail.select_folder(app::Folder::All);
    mail.set_unread_only(true);
    assert!(mail.reader.messages.iter().all(|row| !mail.reader.is_read(row.index)));
    assert!(mail.reader.contains(3));
}

#[test]
fn message_search_ignores_stale_results_and_reports_body_errors() {
    use std::collections::HashSet;

    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    search_messages(&context, &mut mail, "line 4");
    let old_generation = mail.loader.search_generation();
    search_messages(&context, &mut mail, "dave");
    mail.loader
        .sender
        .send(loading::Event::Search(old_generation, "line 4".into(), Ok(HashSet::from([0]))))
        .unwrap();
    mail.poll(&context);
    assert_eq!(mail.reader.selected_message, Some(3));
    let old_generation = mail.loader.search_generation();
    search_messages(&context, &mut mail, "");
    mail.loader
        .sender
        .send(loading::Event::Search(old_generation, "dave".into(), Ok(HashSet::from([3]))))
        .unwrap();
    mail.poll(&context);
    assert_eq!(mail.reader.messages.len(), 4);
    assert!(!mail.loader.searching);

    search_messages(&context, &mut mail, "line 4");
    let old_generation = mail.loader.search_generation();
    let (other, _) = packet_tests::load();
    mail.open(other.path().join("TEST.QWK"), &context);
    wait(&mut mail, &context);
    search_messages(&context, &mut mail, "line 4");
    mail.loader
        .sender
        .send(loading::Event::Search(old_generation, "line 4".into(), Ok(HashSet::from([0]))))
        .unwrap();
    mail.poll(&context);
    assert_eq!(mail.reader.messages.iter().map(|row| row.index).collect::<Vec<_>>(), [2]);

    Arc::make_mut(mail.reader.package.as_mut().unwrap()).descriptors.clear();
    search_messages(&context, &mut mail, "unreadable body");
    assert!(
        mail.error.as_deref().is_some_and(|error| error.contains("Unable to search message")),
        "{:?}",
        mail.error
    );
}

#[test]
fn reader_escape_deselects_without_clearing_filter_or_dismissing_dialogs() {
    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    mail.reader.filter = "Coffee".into();
    mail.set_focus(Pane::Content, &context);
    for shape in [icy_engine::Shape::Lines, icy_engine::Shape::Rectangle] {
        let mut selection = icy_engine::Selection::new((0, 0));
        selection.shape = shape;
        mail.screen.terminal.screen.lock().set_selection(selection).unwrap();
        frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert!(mail.screen.terminal.screen.lock().selection().is_none());
        assert_eq!(mail.reader.filter, "Coffee");
    }
    mail.screen.terminal.screen.lock().set_selection(icy_engine::Selection::new((0, 0))).unwrap();
    mail.modal = Some(app::Modal::Shortcuts);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.modal.is_none());
    assert!(mail.screen.terminal.screen.lock().selection().is_some());
    assert_eq!(mail.reader.filter, "Coffee");

    settle(&context, &mut mail, size);
    context.memory_mut(|memory| memory.request_focus(egui::Id::new("mail-search")));
    settle(&context, &mut mail, size);
    assert_eq!(context.memory(|memory| memory.focused()), Some(egui::Id::new("mail-search")));
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.reader.filter.is_empty(), "search Escape must still clear the filter");
    assert_eq!(mail.focus, Pane::Messages);
}

#[test]
fn reader_selection_coordinates_include_partial_cell_scroll_before_rounding() {
    let context = egui::Context::default();
    let (_dir, mail) = loaded(&context);
    for scale in [0.75, 1.0, 2.0] {
        for scan_lines in [false, true] {
            {
                let mut info = mail.screen.terminal.render_info.write();
                info.bounds_x = 100.0;
                info.bounds_y = 200.0;
                info.viewport_x = 12.0;
                info.viewport_y = 8.0;
                info.display_scale = scale;
                info.font_width = 8.0;
                info.font_height = 16.0;
                info.scan_lines = scan_lines;
            }
            mail.screen.terminal.update_scroll_viewport([4.0 * scale, 8.0 * scale, 100.0, 100.0], scale);
            let y = if scan_lines { 24.0 } else { 12.0 };
            assert_eq!(mail.cell(egui::pos2(112.0 + 6.0 * scale, 208.0 + y * scale)), Some((1, 1).into()));
            assert_eq!(mail.cell(egui::pos2(-1000.0, -1000.0)), Some((0, 0).into()));
        }
    }
}

#[test]
fn command_wheel_zooms_the_message() {
    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    // The classic terminal view is what this test is about.
    mail.reading_mode = icy_mail::options::ReadingMode::Classic;
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    let pointer = mail.content_rect.center();
    let before = mail.screen.zoom;
    frame(
        &context,
        &mut mail,
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
    assert!(matches!(mail.settings.scaling_mode, ScalingMode::Manual(zoom) if zoom > before));
}

#[test]
fn reader_drag_uses_release_position_in_both_directions_and_clamps_to_body() {
    for reverse in [false, true] {
        let context = egui::Context::default();
        let (_dir, mut mail) = loaded(&context);
        // The classic terminal view is what this test is about.
        mail.reading_mode = icy_mail::options::ReadingMode::Classic;
        let size = egui::vec2(1100.0, 760.0);
        settle(&context, &mut mail, size);
        mail.screen = icy_engine_gui::egui::screen::ScreenView::new(icy_mail::reader::render_body(b"HELLO WORLD\nSECOND LINE").unwrap());
        settle(&context, &mut mail, size);
        // Headless frames do not execute the GPU callback that supplies mouse geometry.
        {
            let mut info = mail.screen.terminal.render_info.write();
            info.bounds_x = mail.content_rect.left();
            info.bounds_y = mail.content_rect.top();
            info.display_scale = 1.0;
            info.font_width = 8.0;
            info.font_height = 16.0;
        }
        let cell = |x: f32, y: f32| mail.content_rect.min + egui::vec2((x + 0.5) * 8.0, (y + 0.5) * 16.0);
        let left = cell(0.0, 0.0);
        let right = cell(10.0, 1.0);
        let middle = cell(5.0, 0.0);
        let (start, end) = if reverse { (right, left) } else { (left, right) };
        frame(&context, &mut mail, size, pointer(start, true));
        frame(&context, &mut mail, size, vec![egui::Event::PointerMoved(middle)]);
        frame(&context, &mut mail, size, pointer(end, false));
        let selection = mail.screen.terminal.screen.lock().selection().unwrap();
        let (anchor, lead) = if reverse { ((10, 1), (0, 0)) } else { ((0, 0), (10, 1)) };
        assert_eq!(selection.anchor, anchor.into());
        assert_eq!(selection.lead, lead.into());
        assert!(mail.selection_anchor.is_none(), "release must finish the drag");
        assert!(selection.is_inside((5, 0)));
        assert!(selection.is_inside((5, 1)));
        assert_eq!(
            mail.screen.terminal.screen.lock().copy_text().as_deref(),
            Some("HELLO WORLD\nSECOND LINE"),
            "copy must contain the same text regardless of drag direction"
        );

        frame(&context, &mut mail, size, pointer(middle, true));
        assert!(
            mail.screen.terminal.screen.lock().selection().is_none(),
            "mouse-down must immediately clear the selection"
        );
        frame(&context, &mut mail, size, pointer(middle, false));
        assert!(
            mail.screen.terminal.screen.lock().selection().is_none(),
            "a plain click must clear the previous drag selection"
        );
        for position in [left, right, middle] {
            frame(&context, &mut mail, size, pointer(position, true));
            frame(&context, &mut mail, size, pointer(position, false));
            assert!(
                mail.screen.terminal.screen.lock().selection().is_none(),
                "quick clicks at different positions are single clicks, not word/line selections"
            );
        }

        frame(&context, &mut mail, size, pointer(middle, true));
        frame(&context, &mut mail, size, vec![egui::Event::PointerMoved(start)]);
        let outside = mail.content_rect.max + egui::vec2(4000.0, 4000.0);
        frame(&context, &mut mail, size, pointer(outside, false));
        let screen = mail.screen.terminal.screen.lock();
        assert_eq!(screen.selection().unwrap().lead, (screen.width() - 1, screen.height() - 1).into());
    }
}

#[test]
fn search_matches_are_highlighted_in_the_list_and_header() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    assert!(highlighted(&settle(&context, &mut mail, size)).is_empty());
    mail.reader.filter = "COFFEE".into();
    mail.reader.rebuild_messages();
    let output = settle(&context, &mut mail, size);
    let marked = highlighted(&output);
    assert_eq!(
        marked.iter().filter(|text| *text == "Coffee").count(),
        3,
        "two list rows and the header: {marked:?}"
    );
    mail.reader.filter = "ali".into();
    mail.reader.rebuild_messages();
    let marked = highlighted(&settle(&context, &mut mail, size));
    assert!(marked.iter().all(|text| text == "ali") && marked.len() >= 2, "{marked:?}");
}

#[test]
fn settings_preview_live_cancel_restores_and_ok_persists() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)]);
    assert!(matches!(mail.modal, Some(app::Modal::Settings)));
    let output = settle(&context, &mut mail, size);
    label(&output, "Theme");
    click_label(&context, &mut mail, size, "Fit Width");
    click_label(&context, &mut mail, size, "200%");
    assert_eq!(mail.settings.scaling_mode, ScalingMode::Manual(2.0), "changes preview live");
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.modal.is_none());
    assert_eq!(mail.settings.scaling_mode, ScalingMode::FitWidth, "cancel restores the settings");
    assert!(!dir.path().join("settings.toml").exists());

    frame(&context, &mut mail, size, vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    click_label(&context, &mut mail, size, "Monitor");
    let output = settle(&context, &mut mail, size);
    label(&output, "Gamma");
    click_label(&context, &mut mail, size, "General");
    click_label(&context, &mut mail, size, "Fit Width");
    click_label(&context, &mut mail, size, "150%");
    click_label(&context, &mut mail, size, "OK");
    assert!(mail.modal.is_none());
    assert_eq!(mail.settings.scaling_mode, ScalingMode::Manual(1.5));
    let fresh = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    assert_eq!(fresh.settings.scaling_mode, ScalingMode::Manual(1.5), "saved settings are loaded");

    // Zoom changed from the status bar is remembered as well.
    click_label(&context, &mut mail, size, "150%");
    click_label(&context, &mut mail, size, "100%");
    frame(&context, &mut mail, size, vec![]);
    let fresh = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    assert_eq!(fresh.settings.scaling_mode, ScalingMode::Manual(1.0));
}

#[test]
fn list_or_thread_view_is_remembered() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    assert_eq!(mail.reader.view_mode, ViewMode::List);
    mail.set_mode(ViewMode::Threads);
    frame(&context, &mut mail, size, vec![]);

    let mut fresh = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    assert_eq!(fresh.reader.view_mode, ViewMode::Threads, "view mode is loaded with the settings");
    fresh.open(dir.path().join("TEST.QWK"), &context);
    wait(&mut fresh, &context);
    assert_eq!(fresh.reader.view_mode, ViewMode::Threads, "opening a packet keeps the view mode");

    fresh.set_mode(ViewMode::List);
    frame(&context, &mut fresh, size, vec![]);
    let reopened = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    assert_eq!(reopened.reader.view_mode, ViewMode::List);
}

#[test]
fn composer_picks_taglines_and_recipients_from_the_lists() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, _package) = packet_tests::load();
    std::fs::write(dir.path().join("taglines.txt"), "First saying\nSecond saying\n").unwrap();
    let mut mail = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    mail.open(dir.path().join("TEST.QWK"), &context);
    wait(&mut mail, &context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::N, egui::Modifiers::COMMAND)]);
    let tagline = mail.composer.as_ref().unwrap().draft.tagline.clone();
    assert!(["First saying", "Second saying"].contains(&tagline.as_str()), "random tagline: {tagline:?}");
    assert!(!mail.composer.as_ref().unwrap().dirty(), "the random tagline is not an edit");
    let output = settle(&context, &mut mail, size);
    label(&output, &format!("... {tagline}"));

    frame(&context, &mut mail, size, vec![key(egui::Key::T, egui::Modifiers::COMMAND)]);
    assert!(matches!(mail.modal, Some(app::Modal::Taglines)));
    let output = settle(&context, &mut mail, size);
    label(&output, "Choose a Tagline");
    click_label(&context, &mut mail, size, "... First saying");
    click_label(&context, &mut mail, size, "Use Tagline");
    assert!(mail.modal.is_none());
    assert_eq!(mail.composer.as_ref().unwrap().draft.tagline, "First saying");

    frame(&context, &mut mail, size, vec![key(egui::Key::T, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    click_label(&context, &mut mail, size, "No Tagline");
    assert_eq!(mail.composer.as_ref().unwrap().draft.tagline, "");
    label(&settle(&context, &mut mail, size), "No tagline");

    // A new tagline is stored in the list and can be used right away.
    frame(&context, &mut mail, size, vec![key(egui::Key::T, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    click_label(&context, &mut mail, size, "New Tagline");
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![text("... Fresh one")]);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    let output = settle(&context, &mut mail, size);
    label(&output, "... Fresh one");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("taglines.txt")).unwrap(),
        "First saying\nSecond saying\nFresh one\n"
    );
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert!(mail.modal.is_none(), "Enter uses the selected tagline");
    assert_eq!(mail.composer.as_ref().unwrap().draft.tagline, "Fresh one");

    // The address book fills in the recipient.
    frame(&context, &mut mail, size, vec![key(egui::Key::B, egui::Modifiers::COMMAND)]);
    assert!(matches!(mail.modal, Some(app::Modal::AddressBook)));
    let output = settle(&context, &mut mail, size);
    label(&output, "The address book is empty");
    click_label(&context, &mut mail, size, "New Contact");
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![text("Zed Zero")]);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    label(&settle(&context, &mut mail, size), "Zed Zero");
    assert_eq!(std::fs::read_to_string(dir.path().join("addressbook.txt")).unwrap(), "Zed Zero\n\n\n");
    click_label(&context, &mut mail, size, "Use as Recipient");
    assert!(mail.modal.is_none());
    assert_eq!(mail.composer.as_ref().unwrap().draft.to, "Zed Zero");

    settle(&context, &mut mail, size);
    assert!(mail.composer.as_ref().unwrap().editor.has_focus(), "the text has the keyboard again");
    frame(&context, &mut mail, size, vec![text("Hello")]);
    mail.composer.as_mut().unwrap().draft.subject = "Greetings".into();
    click_label(&context, &mut mail, size, "Save Draft");
    let draft = &mail.drafts.as_ref().unwrap().drafts()[0];
    assert_eq!(draft.tagline, "Fresh one");
    assert!(draft.text().ends_with("\n\n... Fresh one"), "{:?}", draft.text());
}

#[test]
fn reader_keeps_authors_and_taglines_and_writes_to_contacts() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    assert!(mail.message_selected());
    frame(&context, &mut mail, size, vec![key(egui::Key::A, egui::Modifiers::SHIFT)]);
    assert_eq!(std::fs::read_to_string(dir.path().join("addressbook.txt")).unwrap(), "alice\n\n\n");
    assert!(mail.notice.as_ref().is_some_and(|notice| notice.text == "alice added to the address book"));
    frame(&context, &mut mail, size, vec![key(egui::Key::A, egui::Modifiers::SHIFT)]);
    assert!(mail.notice.as_ref().is_some_and(|notice| notice.text.contains("already")));

    frame(&context, &mut mail, size, vec![key(egui::Key::T, egui::Modifiers::NONE)]);
    assert!(mail.notice.as_ref().is_some_and(|notice| notice.text == "This message has no tagline"));
    assert!(!dir.path().join("taglines.txt").exists());

    frame(
        &context,
        &mut mail,
        size,
        vec![key(egui::Key::T, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)],
    );
    assert!(matches!(mail.modal, Some(app::Modal::Taglines)));
    let output = settle(&context, &mut mail, size);
    label(&output, "No taglines yet");
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.modal.is_none());

    // Enter in the address book starts a message to the selected contact.
    frame(&context, &mut mail, size, vec![key(egui::Key::A, egui::Modifiers::NONE)]);
    assert!(matches!(mail.modal, Some(app::Modal::AddressBook)));
    label(&settle(&context, &mut mail, size), "alice");
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert!(mail.modal.is_none());
    let composer = mail.composer.as_ref().expect("a new message");
    assert_eq!(composer.draft.to, "alice");
    assert_eq!(composer.draft.ref_number, 0);
}

#[test]
fn compose_reply_edit_delete_and_export_from_ui() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    // The quote panel opens with the reply; Ctrl+A quotes everything from the attribution on.
    frame(&context, &mut mail, size, vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
    click_label(&context, &mut mail, size, "Save Draft");
    assert_eq!(mail.drafts.as_ref().unwrap().drafts().len(), 1);
    assert!(mail.notice.as_ref().is_some_and(|notice| notice.text.contains("not sent")));
    click_label(&context, &mut mail, size, "Outbox");
    let output = settle(&context, &mut mail, size);
    assert!(output
        .shapes
        .iter()
        .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("upload it to your BBS"))));
    let draft = mail.drafts.as_ref().unwrap().drafts()[0].clone();
    let draft = &draft;
    assert_eq!(draft.to, "alice");
    assert_eq!(draft.ref_number, 10);
    assert!(draft.body.starts_with("On "), "{:?}", draft.body);
    assert!(draft.body.contains("wrote:\n A> line 0\n A> line 1"), "{:?}", draft.body);
    let packet = dir.path().join("OUT.rep");
    mail.drafts.as_ref().unwrap().export_rep(&packet).unwrap();
    assert!(packet.exists());
    let reopened = icy_mail::drafts::DraftStore::open_in(mail.path.as_ref().unwrap(), mail.reader.package.as_ref().unwrap(), dir.path()).unwrap();
    assert_eq!(reopened.drafts()[0], *draft);
    let id = draft.id;
    mail.edit_draft(&context, id);
    click_label(&context, &mut mail, size, "Delete Draft");
    click_label(&context, &mut mail, size, "Delete");
    assert!(mail.drafts.as_ref().unwrap().drafts().is_empty());
    assert!(mail.composer.is_none());
}

#[test]
fn exporting_replies_shows_the_saved_file_and_next_step() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    let path = dir.path().join("OUT.rep");
    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
    click_label(&context, &mut mail, size, "Save Draft");
    mail.drafts.as_ref().unwrap().export_rep(&path).unwrap();
    mail.loader.export_picking = true;
    mail.loader.sender.send(loading::Event::Exported(Some((path.clone(), Ok(()))))).unwrap();
    let output = settle(&context, &mut mail, size);
    assert!(!mail.loader.export_picking);
    assert!(path.exists());
    assert_eq!(mail.draft_count(), 1, "export must not delete the draft");
    assert!(matches!(&mail.modal, Some(app::Modal::Exported(saved)) if saved == &path));
    label(&output, "Reply packet ready");
    label(&output, "Open Folder");
    assert!(output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text)
            if text.galley.text().contains(&path.display().to_string())
                && text.galley.text().contains("not been sent")
                && text.galley.text().contains("drafts remain in the Outbox"))
    }));
    let narrow = settle(&context, &mut mail, egui::vec2(390.0, 640.0));
    label(&narrow, "Open Folder");
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.modal.is_none());
    let output = settle(&context, &mut mail, size);
    label(&output, "1 draft in Outbox");
}

#[test]
fn new_and_forward_create_distinct_drafts() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    click_label(&context, &mut mail, size, "New");
    // Saving waits until the message is complete; the reason is shown on the disabled button.
    click_label(&context, &mut mail, size, "Save Draft");
    assert!(mail.drafts.as_ref().unwrap().drafts().is_empty());
    let composer = mail.composer.as_mut().unwrap();
    composer.draft.subject = "Hello".into();
    composer.draft.body = "Text".into();
    click_label(&context, &mut mail, size, "Save Draft");
    let draft = &mail.drafts.as_ref().unwrap().drafts()[0];
    assert_eq!(draft.to, "ALL");
    assert_eq!(draft.conference, 1);
    assert_eq!(draft.ref_number, 0);
    frame(&context, &mut mail, size, vec![key(egui::Key::L, egui::Modifiers::COMMAND)]);
    mail.composer.as_mut().unwrap().draft.to = "bob".into();
    click_label(&context, &mut mail, size, "Save Draft");
    let draft = &mail.drafts.as_ref().unwrap().drafts()[1];
    assert_eq!(draft.to, "bob");
    assert_eq!(draft.subject, "Fwd: Coffee machine");
    assert!(draft.body.contains("> line 0"));
    assert_eq!(draft.ref_number, 0);
}

fn text(text: &str) -> egui::Event {
    egui::Event::Text(text.into())
}

#[test]
fn message_editor_types_colors_and_inserts_cp437_characters() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    assert!(mail.composer.as_ref().unwrap().editor.has_focus());
    assert!(mail.composer.as_ref().unwrap().editor.quotes.open);
    // Escape closes the quote panel first, not the message.
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.composer.is_some());
    assert!(!mail.composer.as_ref().unwrap().editor.quotes.open);

    frame(&context, &mut mail, size, vec![text("Hi")]);
    frame(&context, &mut mail, size, vec![key(egui::Key::K, egui::Modifiers::COMMAND)]);
    frame(&context, &mut mail, size, vec![text("e")]);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![text("!")]);
    frame(&context, &mut mail, size, vec![key(egui::Key::G, egui::Modifiers::COMMAND)]);
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![text("\u{20ac}")]);
    let composer = mail.composer.as_ref().unwrap();
    assert_eq!(composer.editor.editor.plain_text(), "Hi!\u{2592}");
    assert_eq!(composer.draft.body, "Hi\x1b[0;1;33m!\u{2592}\x1b[0m");
    assert!(composer.editor.status.as_deref().is_some_and(|status| status.contains("not a CP437 character")));

    frame(&context, &mut mail, size, vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(mail.composer.as_ref().unwrap().editor.editor.plain_text(), "Hi!");
    frame(&context, &mut mail, size, vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(mail.composer.as_ref().unwrap().editor.editor.plain_text(), "Hi");
    frame(&context, &mut mail, size, vec![key(egui::Key::Y, egui::Modifiers::COMMAND)]);
    click_label(&context, &mut mail, size, "Save Draft");
    let store = mail.drafts.as_ref().unwrap();
    let draft = &store.drafts()[0];
    assert_eq!(draft.body, "Hi\x1b[0;1;33m!\x1b[0m");
    assert!(store.issues(draft).is_empty(), "{:?}", store.issues(draft));
}

#[test]
fn quote_window_inserts_picked_lines_and_escape_leaves_the_editor() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    for event in [
        key(egui::Key::ArrowDown, egui::Modifiers::NONE),
        key(egui::Key::Enter, egui::Modifiers::NONE),
        key(egui::Key::Enter, egui::Modifiers::NONE),
    ] {
        frame(&context, &mut mail, size, vec![event]);
    }
    let composer = mail.composer.as_ref().unwrap();
    assert_eq!(composer.editor.editor.plain_text(), " A> line 0\n A> line 1\n");
    assert!(composer.dirty());
    frame(&context, &mut mail, size, vec![key(egui::Key::Q, egui::Modifiers::COMMAND)]);
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![]);
    assert!(matches!(mail.modal, Some(app::Modal::Discard(_))), "unsaved text asks before it is dropped");
}

#[test]
fn editor_panels_find_pick_colors_and_characters_with_the_mouse() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![text("one two one")]);
    frame(&context, &mut mail, size, vec![key(egui::Key::Home, egui::Modifiers::COMMAND)]);

    // Find is a regular text field; Enter searches and keeps it open, Escape returns to the text.
    frame(&context, &mut mail, size, vec![key(egui::Key::F, egui::Modifiers::COMMAND)]);
    frame(&context, &mut mail, size, vec![]);
    frame(&context, &mut mail, size, vec![text("two")]);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    let editor = &mail.composer.as_ref().unwrap().editor;
    assert_eq!(editor.find.as_deref(), Some("two"));
    assert_eq!(editor.editor.selected_text().as_deref(), Some("two"));
    assert_eq!(editor.editor.plain_text(), "one two one", "typing went to the find field");
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    settle(&context, &mut mail, size);
    let editor = &mail.composer.as_ref().unwrap().editor;
    assert!(editor.find.is_none(), "{:?}", editor.find);
    assert!(editor.has_focus());
    assert!(mail.composer.is_some());

    // The color button opens the picker; keys still work in it and Apply sets the color.
    frame(&context, &mut mail, size, vec![key(egui::Key::End, egui::Modifiers::COMMAND)]);
    click_label(&context, &mut mail, size, "Aa");
    settle(&context, &mut mail, size);
    assert!(mail.composer.as_ref().unwrap().editor.colors.is_some());
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)]);
    click_label(&context, &mut mail, size, "Apply");
    let editor = &mail.composer.as_ref().unwrap().editor;
    assert!(editor.colors.is_none());
    assert_eq!(editor.editor.attr().fg, 8);

    // Clicking a glyph in the character table inserts it and keeps the table open.
    click_label(&context, &mut mail, size, "Characters");
    settle(&context, &mut mail, size);
    let grid = mail.composer.as_ref().unwrap().editor.chars.grid;
    let cell = grid.width() / 16.0;
    let position = grid.min + egui::vec2(11.5, 13.5) * cell;
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(position, pressed));
    }
    settle(&context, &mut mail, size);
    let editor = &mail.composer.as_ref().unwrap().editor;
    assert!(editor.chars.open && editor.has_focus());
    assert_eq!(editor.editor.plain_text(), "one two one\u{2588}");
    frame(&context, &mut mail, size, vec![text("x")]);
    assert_eq!(mail.composer.as_ref().unwrap().draft.body, "one two one\x1b[0;1;30m\u{2588}x\x1b[0m");
}

#[test]
fn color_picker_buttons_fit_side_by_side() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    click_label(&context, &mut mail, size, "Aa");
    let output = settle(&context, &mut mail, size);
    let popup = context
        .memory(|memory| memory.area_rect(egui::Id::new("editor-colors")))
        .expect("color picker is open");
    let buttons: Vec<_> = ["editor-default", "editor-cancel", "editor-apply"]
        .into_iter()
        .map(|id| label(&output, &icy_mail::LANGUAGE_LOADER.get(id)))
        .collect();
    for (index, button) in buttons.iter().enumerate() {
        assert!(popup.contains_rect(*button), "{button:?} outside {popup:?}");
        for other in &buttons[index + 1..] {
            assert!(!button.intersects(*other), "{button:?} overlaps {other:?}");
        }
    }
}

#[test]
fn thread_twisty_and_arrow_keys_collapse_and_expand_threads() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    mail.set_mode(ViewMode::Threads);
    mail.reader.select_message(0);
    let output = settle(&context, &mut mail, size);
    assert_eq!(count(&output, "Re: Amiga demos"), 1);
    let ones = count(&output, "1");
    let subject = label(&output, "Amiga demos");
    let twisty = egui::pos2(subject.left() - 7.0, subject.center().y);
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(twisty, pressed));
    }
    assert!(mail.reader.is_collapsed(2), "clicking the triangle collapses the thread");
    assert_eq!(mail.reader.selected_message, Some(0), "the triangle does not change the selection");
    let output = settle(&context, &mut mail, size);
    assert_eq!(count(&output, "Re: Amiga demos"), 0);
    assert_eq!(count(&output, "1"), ones + 1, "a collapsed thread shows its reply count");
    assert_eq!(mail.reader.all_messages().len(), 4);

    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(twisty, pressed));
    }
    assert!(!mail.reader.is_collapsed(2));

    mail.set_focus(Pane::Messages, &context);
    mail.reader.select_message(1);
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.selected_message, Some(0), "Left moves to the parent");
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)]);
    assert!(mail.reader.is_collapsed(0), "Left collapses the thread");
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)]);
    assert!(!mail.reader.is_collapsed(0), "Right expands the thread");
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.selected_message, Some(1), "Right moves to the first reply");
}

#[test]
fn bulletins_folder_lists_and_shows_packet_files() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    let unread = mail.reader.unread_count();
    assert!(mail.folders().contains(&app::Folder::Bulletins));
    click_label(&context, &mut mail, size, "Bulletins");
    assert_eq!(mail.folder, app::Folder::Bulletins);
    assert_eq!(mail.selected_file, Some(0));
    frame(&context, &mut mail, size, vec![]);
    wait(&mut mail, &context);
    let output = settle(&context, &mut mail, size);
    for title in [
        "Welcome screen",
        "News",
        "Bulletin 2",
        "Bulletin 10",
        "New files",
        "Goodbye screen",
        "NEWFILES.DAT",
    ] {
        assert!(count(&output, title) > 0, "{title} missing");
    }
    let first_line = |mail: &app::MailApp, len: i32| -> String {
        let screen = mail.screen.terminal.screen.lock();
        (0..len).map(|x| screen.char_at((x, 0).into()).ch).collect()
    };
    assert_eq!(first_line(&mail, 7), "Welcome");

    mail.set_focus(Pane::Messages, &context);
    frame(&context, &mut mail, size, vec![key(egui::Key::End, egui::Modifiers::NONE)]);
    assert_eq!(mail.selected_file, Some(5));
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![]);
    wait(&mut mail, &context);
    assert_eq!(mail.rendered_file, Some(4));
    assert_eq!(first_line(&mail, 4), "DEMO", "PCBoard colour codes are not printed");

    // Message actions do not apply to bulletins.
    frame(&context, &mut mail, size, vec![key(egui::Key::M, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.unread_count(), unread);
    assert!(!mail.message_selected());

    mail.select_folder(app::Folder::All);
    frame(&context, &mut mail, size, vec![]);
    wait(&mut mail, &context);
    assert_eq!(mail.rendered_file, None, "leaving the folder shows the message again");
    assert_eq!(mail.rendered, mail.reader.selected_message);
}

fn wait(mail: &mut app::MailApp, context: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        mail.poll(context);
        if mail.loading.is_none() && !mail.body_loading {
            break;
        }
        assert!(Instant::now() < deadline, "mail worker timed out");
        std::thread::yield_now();
    }
    assert!(mail.error.is_none(), "{:?}", mail.error);
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
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

fn frame(context: &egui::Context, mail: &mut app::MailApp, size: egui::Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
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
            time: Some(time),
            events,
            modifiers,
            ..Default::default()
        },
        |context| mail.show(context),
    )
}

fn label(output: &egui::FullOutput, label: &str) -> egui::Rect {
    output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                shape.clip_rect.contains_rect(bounds).then_some(bounds)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing visible label: {label}"))
}

#[test]
fn file_loading_populates_an_initially_empty_reader_and_reports_errors() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, _package) = packet_tests::load();
    let path = dir.path().join("TEST.QWK");
    let mut mail = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    frame(&context, &mut mail, egui::vec2(1100.0, 760.0), vec![]);
    mail.open(path.clone(), &context);
    assert!(mail.loading.is_some());
    wait(&mut mail, &context);
    assert_eq!(mail.reader.messages.len(), 4);
    assert_eq!(mail.reader.selected_message, Some(0));
    assert_eq!(mail.path.as_ref(), Some(&path));
    let original = mail.reader.package.clone().unwrap();
    mail.open(dir.path().join("missing.qwk"), &context);
    let deadline = Instant::now() + Duration::from_secs(5);
    while mail.loading.is_some() {
        mail.poll(&context);
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(mail.error.as_ref().unwrap().contains("missing.qwk"));
    assert!(Arc::ptr_eq(&original, mail.reader.package.as_ref().unwrap()));
    if let Some(directory) = std::env::var_os("ICY_EGUI_SCREENSHOTS") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::copy(path, PathBuf::from(directory).join("example.qwk")).unwrap();
    }
}

#[test]
fn large_bulletins_scroll_between_sections_without_page_controls() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let dir = packet_tests::TempDir::new();
    let mut data = b"FIRST\n".to_vec();
    data.extend(b"filler\n".repeat(icy_mail::reader::FILE_PAGE_LINES - 1));
    data.extend(b"LAST\n");
    let path = packet_tests::write_packet_with_newfiles(dir.path(), Some(&data));
    let mut mail = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    mail.open(path, &context);
    wait(&mut mail, &context);
    mail.select_folder(app::Folder::Bulletins);
    mail.selected_file = mail.reader.package.as_ref().unwrap().files.iter().position(|file| file.name == "NEWFILES.DAT");
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    wait(&mut mail, &context);
    let output = settle(&context, &mut mail, size);
    assert_eq!(mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch, 'F');
    for text in ["Previous page", "Next page", "Page 1 of 2"] {
        assert_eq!(count(&output, text), 0, "{text} should not be visible");
    }

    mail.set_focus(Pane::Content, &context);
    mail.screen.scroll_to = Some(egui::vec2(0.0, f32::MAX));
    settle(&context, &mut mail, size);
    assert!(mail.screen.max_offset.y > 0.0);
    assert!((mail.screen.offset.y - mail.screen.max_offset.y).abs() <= 1.0);
    let wheel = |position, delta| {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let position = mail.content_rect.center();
    frame(&context, &mut mail, size, wheel(position, -80.0));
    assert_eq!(mail.selected_file_page, 1, "scrolling past the end loads the following section");
    wait(&mut mail, &context);
    settle(&context, &mut mail, size);
    assert_eq!(mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch, 'L');
    mail.screen.scroll_to = Some(egui::Vec2::ZERO);
    settle(&context, &mut mail, size);
    let position = mail.content_rect.center();
    frame(&context, &mut mail, size, wheel(position, 80.0));
    assert_eq!(mail.selected_file_page, 0, "scrolling above the start loads the previous section");
    wait(&mut mail, &context);
    settle(&context, &mut mail, size);
    assert!(
        mail.screen.max_offset.y - mail.screen.offset.y < 100.0,
        "the previous section opens near its end, allowing for the wheel delta"
    );
    mail.screen.scroll_to = Some(egui::vec2(0.0, f32::MAX));
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::PageDown, egui::Modifiers::NONE)]);
    assert_eq!(mail.selected_file_page, 1, "Page Down also continues into the next section");
    wait(&mut mail, &context);
    settle(&context, &mut mail, size);
    mail.screen.scroll_to = Some(egui::Vec2::ZERO);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::PageUp, egui::Modifiers::NONE)]);
    assert_eq!(mail.selected_file_page, 0, "Page Up returns to the previous section");
    wait(&mut mail, &context);
    mail.screen.scroll_to = Some(egui::vec2(0.0, f32::MAX));
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Space, egui::Modifiers::NONE)]);
    assert_eq!(mail.selected_file_page, 1, "Space continues the bulletin before moving on to mail");
}

#[test]
fn new_window_shortcut_uses_exact_modifiers_and_registers_native_viewport() {
    let context = egui::Context::default();
    context.set_embed_viewports(false);
    appearance::apply(&context);
    let (dir, _package) = packet_tests::load();
    let mut mail = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![]);
    let output = frame(
        &context,
        &mut mail,
        size,
        vec![key(egui::Key::N, egui::Modifiers::COMMAND | egui::Modifiers::ALT | egui::Modifiers::SHIFT)],
    );
    assert_eq!(output.viewport_output.len(), 1);
    let output = frame(
        &context,
        &mut mail,
        size,
        vec![key(egui::Key::N, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)],
    );
    assert_eq!(output.viewport_output.len(), 2);
    let child = output.viewport_output.iter().find(|(id, _)| **id != egui::ViewportId::ROOT).unwrap().0;
    assert!(output.viewport_output[child].viewport_ui_cb.is_some());
    let output = frame(&context, &mut mail, size, vec![key(egui::Key::W, egui::Modifiers::COMMAND)]);
    assert!(mail.closed);
    assert!(output.viewport_output[child].commands.contains(&egui::ViewportCommand::Close));
}

#[test]
fn stale_package_and_body_results_cannot_replace_current_mail() {
    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    mail.loader.package_generation = 5;
    mail.loader.body_generation = 9;
    let original = mail.reader.package.clone().unwrap();
    mail.loader
        .sender
        .send(loading::Event::Package(4, PathBuf::from("stale.qwk"), Err("stale failure".into())))
        .unwrap();
    mail.loader
        .sender
        .send(loading::Event::Body(
            8,
            icy_mail::reader::render_body(b"STALE").map_err(|error| error.to_string()),
        ))
        .unwrap();
    mail.poll(&context);
    assert!(mail.error.is_none());
    assert!(Arc::ptr_eq(&original, mail.reader.package.as_ref().unwrap()));
    assert_eq!(mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch, 'l');
    mail.loader
        .sender
        .send(loading::Event::Package(5, PathBuf::from("bad.qwk"), Err("invalid archive".into())))
        .unwrap();
    mail.poll(&context);
    assert!(mail.error.as_ref().unwrap().contains("invalid archive"));
    assert!(Arc::ptr_eq(&original, mail.reader.package.as_ref().unwrap()));
}

#[test]
fn selection_change_and_empty_filter_drop_old_body_results() {
    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    mail.reader.navigate(Pane::Messages, NavigateDirection::Last);
    mail.poll(&context);
    let generation = mail.loader.body_generation;
    mail.reader.filter = "NO MATCH".into();
    mail.reader.rebuild_messages();
    mail.poll(&context);
    assert!(mail.reader.selected_message.is_none());
    mail.loader
        .sender
        .send(loading::Event::Body(
            generation,
            icy_mail::reader::render_body(b"STALE").map_err(|error| error.to_string()),
        ))
        .unwrap();
    mail.poll(&context);
    assert!(!mail.body_loading);
    assert_eq!(mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch, ' ');
}

#[test]
fn tables_click_sort_filter_and_preserve_keyboard_focus() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    for _ in 0..3 {
        frame(&context, &mut mail, size, vec![]);
    }
    let output = frame(&context, &mut mail, size, vec![]);
    let retro = label(&output, "Retro").center();
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(retro, pressed));
    }
    assert_eq!(mail.reader.selected_conference, Some(2));
    assert_eq!(mail.reader.messages.len(), 2);
    assert_eq!(mail.focus, Pane::Conferences);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert_eq!(mail.focus, Pane::Messages);
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.selected_message, Some(3));
    frame(&context, &mut mail, size, vec![key(egui::Key::T, egui::Modifiers::COMMAND)]);
    assert_eq!(mail.reader.view_mode, ViewMode::Threads);
    frame(&context, &mut mail, size, vec![key(egui::Key::F, egui::Modifiers::COMMAND)]);
    frame(&context, &mut mail, size, vec![egui::Event::Text("CAROL".into())]);
    assert_eq!(mail.reader.selected_message, Some(2));
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.selected_message, Some(2));
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.reader.filter.is_empty());
    frame(&context, &mut mail, size, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    assert_eq!(mail.focus, Pane::Content);
    frame(&context, &mut mail, size, vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)]);
    assert_eq!(mail.focus, Pane::Messages);
}

#[test]
fn modal_blocks_navigation_and_close_key_does_not_leak() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    mail.error = Some("Invalid package".into());
    frame(
        &context,
        &mut mail,
        egui::vec2(360.0, 240.0),
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
    );
    assert_eq!(mail.reader.selected_message, Some(0));
    frame(
        &context,
        &mut mail,
        egui::vec2(360.0, 240.0),
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(mail.error.is_none());
    assert_eq!(mail.reader.selected_message, Some(0));
}

#[test]
fn responsive_toolbar_has_one_search_field_and_keeps_actions_on_screen() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    for size in [
        egui::vec2(1100.0, 760.0),
        egui::vec2(600.0, 500.0),
        egui::vec2(360.0, 240.0),
        egui::vec2(1100.0, 760.0),
    ] {
        let output = settle(&context, &mut mail, size);
        assert_eq!(count(&output, "Search messages"), 1, "{size:?}: exactly one search field");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        assert!(screen.contains_rect(label(&output, "Search messages")), "{size:?}");
        if size.x >= 900.0 {
            for text in ["Open", "New", "Export Replies"] {
                assert!(screen.contains_rect(label(&output, text)), "{size:?}: {text}");
            }
            assert_eq!(count(&output, "Reply"), 0, "{size:?}: reply lives in the message header");
            assert_eq!(count(&output, "Lines"), 0, "{size:?}: message list has no line count column");
            let toolbar = label(&output, "Open").center().y;
            assert!(
                (label(&output, "Next Unread").center().y - toolbar).abs() < 2.0,
                "{size:?}: next unread sits in the toolbar"
            );
            let progress = label(&output, "1 of 4 read · 3 unread");
            assert!(progress.top() > size.y - 30.0, "{size:?}: reading progress lives in the status bar");
        } else {
            assert_eq!(count(&output, "New"), 0, "{size:?}: compact toolbar shows icons only");
        }
    }
}

#[test]
fn next_unread_button_shows_progress_and_opens_the_message_on_narrow_windows() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    for size in [
        egui::vec2(1100.0, 760.0),
        egui::vec2(600.0, 500.0),
        egui::vec2(360.0, 480.0),
        egui::vec2(360.0, 240.0),
    ] {
        let output = settle(&context, &mut mail, size);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        assert!(screen.contains_rect(label(&output, "Next Unread")), "{size:?}: next unread must be visible");
        assert!(
            screen.contains_rect(label(&output, "1 of 4 read · 3 unread")),
            "{size:?}: progress must be visible"
        );
    }
    let size = egui::vec2(360.0, 480.0);
    mail.set_focus(Pane::Messages, &context);
    click_label(&context, &mut mail, size, "Next Unread");
    assert_eq!(mail.reader.selected_message, Some(1));
    assert_eq!(mail.focus, Pane::Content);
    wait(&mut mail, &context);
    let output = settle(&context, &mut mail, size);
    label(&output, "2 of 4 read · 2 unread");
    let remaining: Vec<_> = mail
        .reader
        .package
        .as_ref()
        .unwrap()
        .infos
        .iter()
        .filter(|info| !mail.reader.is_read(info.index))
        .map(|info| info.index)
        .collect();
    mail.set_read(&context, &remaining, true);
    let output = settle(&context, &mut mail, size);
    label(&output, "4 of 4 read · 0 unread");
    let selected = mail.reader.selected_message;
    click_label(&context, &mut mail, size, "Next Unread");
    assert_eq!(mail.reader.selected_message, selected, "button is disabled once all messages are read");
}

#[test]
fn next_unread_wraps_to_earlier_unread_mail() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    mail.reader.select_message(3);
    settle(&context, &mut mail, size);
    assert!(mail.error.is_none(), "{:?}", mail.error);
    click_label(&context, &mut mail, size, "Next Unread");
    assert_eq!(
        mail.reader.selected_message,
        Some(1),
        "the button should wrap instead of claiming there is no unread mail"
    );
}

#[test]
fn reading_marks_messages_and_next_unread_walks_the_conferences() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    assert!(mail.reader.is_read(0), "the first message was shown");
    assert_eq!(mail.counts.unread, 3);
    frame(&context, &mut mail, size, vec![key(egui::Key::N, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.selected_message, Some(1));
    wait(&mut mail, &context);
    assert_eq!(mail.counts.unread, 2);
    frame(&context, &mut mail, size, vec![key(egui::Key::M, egui::Modifiers::NONE)]);
    assert!(!mail.reader.is_read(1));
    settle(&context, &mut mail, size);
    wait(&mut mail, &context);
    assert!(!mail.reader.is_read(1), "a message marked unread stays unread while it is shown");
    click_label(&context, &mut mail, size, "General");
    assert_eq!(mail.folder, app::Folder::Conference(1));
    assert_eq!(mail.reader.selected_message, Some(1), "the folder opens at its first unread message");
    wait(&mut mail, &context);
    click_label(&context, &mut mail, size, "Next Unread");
    assert_eq!(mail.folder, app::Folder::Conference(2), "next unread continues in the next conference");
    frame(&context, &mut mail, size, vec![key(egui::Key::C, egui::Modifiers::SHIFT)]);
    assert_eq!(mail.counts.conferences.get(&2).copied().unwrap_or(0), 0);
    let package = mail.reader.package.clone().unwrap();
    let state = icy_mail::state::ReadState::open_in(mail.path.as_ref().unwrap(), &package, dir.path()).unwrap();
    assert_eq!(state.indices(&package), std::collections::HashSet::from([0, 2, 3]));
    let mut reopened = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    reopened.open(dir.path().join("TEST.QWK"), &context);
    wait(&mut reopened, &context);
    assert_eq!(
        reopened.reader.selected_message,
        Some(1),
        "a reopened packet starts at the first unread message"
    );
    assert_eq!(reopened.counts.unread, 0, "showing the message marked it read");
}

#[test]
fn outbox_lists_drafts_and_edits_or_deletes_them_from_the_keyboard() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    let message = mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch;
    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![egui::Event::Text("Hi \u{2591}".into())]);
    frame(&context, &mut mail, size, vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    assert!(mail.composer.is_none());
    assert_eq!(mail.draft_count(), 1);
    let output = settle(&context, &mut mail, size);
    label(&output, "1 draft in Outbox");
    click_label(&context, &mut mail, size, "Outbox");
    assert_eq!(mail.folder, app::Folder::Drafts);
    let output = settle(&context, &mut mail, size);
    label(&output, "Re: Coffee machine");
    label(&output, "alice");
    {
        let screen = mail.screen.terminal.screen.lock();
        let codes: Vec<u32> = (0..4).map(|x| screen.char_at((x, 0).into()).ch as u32).collect();
        assert_eq!(
            codes,
            [b'H', b'i', b' ', 0xB0].map(u32::from),
            "the draft is shown as CP437 in the message terminal"
        );
    }
    mail.select_folder(app::Folder::All);
    settle(&context, &mut mail, size);
    assert_eq!(
        mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch,
        message,
        "the inbox shows its message again"
    );
    click_label(&context, &mut mail, size, "Outbox");
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    assert_eq!(mail.focus, Pane::Messages);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert!(mail.composer.as_ref().is_some_and(|composer| composer.existing));
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.composer.is_none(), "an unchanged draft closes without asking");
    frame(&context, &mut mail, size, vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert!(matches!(mail.modal, Some(app::Modal::DeleteDraft(_))));
    click_label(&context, &mut mail, size, "Delete");
    assert_eq!(mail.draft_count(), 0);
    let output = settle(&context, &mut mail, size);
    label(&output, "The outbox is empty");
}

#[test]
fn unsaved_messages_ask_before_they_are_discarded() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::N, egui::Modifiers::COMMAND)]);
    let output = settle(&context, &mut mail, size);
    label(&output, "New Message");
    frame(&context, &mut mail, size, vec![egui::Event::Text("Hello\tworld".into())]);
    let composer = mail.composer.as_ref().unwrap();
    assert!(composer.dirty());
    assert!(!composer.draft.to.contains('\t') && !composer.draft.subject.contains('\t') && !composer.draft.body.contains('\t'));
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(matches!(mail.modal, Some(app::Modal::Discard(app::AfterDiscard::Close))));
    click_label(&context, &mut mail, size, "Keep Editing");
    assert!(mail.composer.is_some() && mail.modal.is_none());
    frame(&context, &mut mail, size, vec![key(egui::Key::W, egui::Modifiers::COMMAND)]);
    assert!(matches!(mail.modal, Some(app::Modal::Discard(app::AfterDiscard::Quit))));
    assert!(!mail.closed);
    click_label(&context, &mut mail, size, "Discard");
    assert!(mail.closed);
    assert!(mail.composer.is_none());
    assert_eq!(mail.draft_count(), 0);
}

#[test]
fn export_points_to_drafts_that_cannot_be_sent_yet() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![key(egui::Key::N, egui::Modifiers::COMMAND)]);
    frame(&context, &mut mail, size, vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    assert_eq!(mail.draft_count(), 0, "incomplete messages are not saved");
    assert!(mail.composer.is_some(), "the message stays open to be completed");
    assert!(mail.notice.as_ref().is_some_and(|notice| notice.text.contains("required")));
    // Drafts stored by older versions may still be incomplete.
    let draft = mail.composer.take().unwrap().draft;
    mail.drafts.as_mut().unwrap().insert(draft).unwrap();
    assert_eq!(mail.draft_count(), 1);
    frame(
        &context,
        &mut mail,
        size,
        vec![key(egui::Key::E, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)],
    );
    assert!(matches!(&mail.modal, Some(app::Modal::ExportProblems(problems)) if problems[0].contains("Subject is required")));
    assert!(!mail.loader.export_picking);
    click_label(&context, &mut mail, size, "Show Outbox");
    assert_eq!(mail.folder, app::Folder::Drafts);
    let output = settle(&context, &mut mail, size);
    label(&output, "Fix before exporting");
}

#[test]
fn welcome_page_opens_and_forgets_recent_packets() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, _mail) = loaded(&context);
    let mut mail = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    let size = egui::vec2(1100.0, 760.0);
    let output = settle(&context, &mut mail, size);
    label(&output, "Open Packet\u{2026}");
    label(&output, "Drop a QWK packet here");
    // Without a stored summary the card is titled by the file.
    click_label(&context, &mut mail, size, "TEST.QWK");
    wait(&mut mail, &context);
    assert!(mail.reader.package.is_some());
    mail.set_starred(&context, 2, true);
    settle(&context, &mut mail, size);

    let mut fresh = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    fresh.recent.as_mut().unwrap().add(&dir.path().join("GONE.QWK")).unwrap();
    let output = settle(&context, &mut fresh, size);
    let texts: Vec<String> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_string()),
            _ => None,
        })
        .collect();
    let has = |needle: &str| texts.iter().any(|text| text.contains(needle));
    assert!(has("GONE.QWK was moved or deleted"), "{texts:?}");
    let title = label(&output, "TEST BBS");
    assert!(label(&output, "2 unread").left() > title.right(), "the unread count sits on the title line");
    assert!(has("TEST.QWK  \u{b7}  "), "file name and size follow: {texts:?}");
    assert!(has("Packed 2020-01-01"), "the packing date reads like a list date: {texts:?}");
    assert!(has("4 messages  \u{b7}  \u{2605} 1 starred"), "{texts:?}");

    let card_center = egui::pos2(title.left() - 60.0 + 280.0, title.center().y + 19.0);
    let forget = egui::pos2(title.left() - 60.0 + 560.0 - 20.0, card_center.y);
    frame(&context, &mut fresh, size, vec![egui::Event::PointerMoved(card_center)]);
    frame(&context, &mut fresh, size, vec![egui::Event::PointerMoved(forget)]);
    for pressed in [true, false] {
        frame(&context, &mut fresh, size, pointer(forget, pressed));
    }
    let recent = fresh.recent.as_ref().unwrap();
    assert!(!recent.packets.iter().any(|path| path.ends_with("TEST.QWK")), "{:?}", recent.packets);
    assert!(recent.summaries.is_empty());
    assert!(fresh.reader.package.is_none());
}

#[test]
fn cp437_header_fields_are_decoded_for_display() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let package = Arc::make_mut(mail.reader.package.as_mut().unwrap());
    let index = package.infos.iter().position(|info| Some(info.index) == mail.reader.selected_message).unwrap();
    package.infos[index].from = b"Andr\x82".as_slice().into();
    package.infos[index].to = b"J\x81rgen".as_slice().into();
    package.infos[index].subject = b"\xb0\xb1\xb2 News".as_slice().into();
    mail.reader.filter = "j\u{fc}rgen".into();
    mail.reader.rebuild_messages();
    assert_eq!(mail.reader.messages.len(), 1, "search matches the decoded text");
    let size = egui::vec2(1100.0, 760.0);
    let output = settle(&context, &mut mail, size);
    label(&output, "Andr\u{e9}");
    label(&output, "J\u{fc}rgen");
    label(&output, "\u{2591}\u{2592}\u{2593} News");
}

#[test]
fn virtualized_table_renders_only_visible_rows_and_reveals_end() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let package = Arc::make_mut(mail.reader.package.as_mut().unwrap());
    let info = package.infos[0].clone();
    let descriptor = package.descriptors[0].clone();
    for index in 4..20_000 {
        let mut next = info.clone();
        next.index = index;
        next.number = index as u32 + 100;
        next.subject = format!("Message {index:05}").into();
        next.date = package.infos[3].date;
        package.infos.push(next);
        package.descriptors.push(descriptor.clone());
    }
    mail.reader.rebuild_conferences();
    mail.reader.rebuild_messages();
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![]);
    frame(&context, &mut mail, size, vec![key(egui::Key::End, egui::Modifiers::NONE)]);
    let output = frame(&context, &mut mail, size, vec![]);
    assert_eq!(mail.reader.selected_message, Some(19_999));
    label(&output, "Message 19999");
    let drawn = output
        .shapes
        .iter()
        .filter(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().starts_with("Message ")))
        .count();
    assert!(drawn < 40, "only visible rows should be painted: {drawn}");
}

/// A packet as large as busy real ones (half a million messages in two conferences).
fn huge(context: &egui::Context, count: usize) -> (packet_tests::TempDir, app::MailApp) {
    let (dir, mut mail) = loaded(context);
    let package = Arc::make_mut(mail.reader.package.as_mut().unwrap());
    let base: Vec<_> = package.infos.clone();
    let descriptor = package.descriptors[0].clone();
    package.infos.reserve(count);
    package.descriptors.reserve(count);
    for index in base.len()..count {
        let mut next = base[index % base.len()].clone();
        next.index = index;
        next.number = index as u32 + 100;
        next.ref_number = if index % 3 == 0 { 0 } else { index as u32 + 99 };
        next.subject = format!("Topic {:05}", index / 7).into();
        next.subject_key = format!("topic {:05}", index / 7);
        next.date += chrono::Duration::minutes(index as i64);
        package.infos.push(next);
        package.descriptors.push(descriptor.clone());
    }
    mail.reader.rebuild_conferences();
    mail.reader.rebuild_messages();
    mail.refresh_counts();
    (dir, mail)
}

fn timed(label: &str, frames: usize, mut run: impl FnMut(usize)) -> Duration {
    let start = Instant::now();
    for frame in 0..frames {
        run(frame);
    }
    let average = start.elapsed() / frames as u32;
    eprintln!("[perf] {label:<28} {:>8.2} ms", average.as_secs_f64() * 1000.0);
    average
}

#[test]
#[ignore = "measures complete body-search throughput"]
fn large_packet_body_search_throughput() {
    let context = egui::Context::default();
    let count = 200_000;
    let (_dir, mut mail) = huge(&context, count);
    let package = mail.reader.package.clone().unwrap();
    for (query, expected) in [("line 2", count - 2), ("line 4", 1), ("not in any body", 0)] {
        let start = Instant::now();
        mail.loader.search(package.clone(), query.into(), &context).unwrap();
        let event = mail.loader.receiver.recv_timeout(Duration::from_secs(60)).unwrap();
        match event {
            loading::Event::Search(_, returned_query, result) => {
                assert_eq!(returned_query, query);
                assert_eq!(result.unwrap().len(), expected);
            }
            _ => panic!("expected body search results"),
        }
        eprintln!(
            "[perf] complete search {query:?}, {count} messages: {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[test]
fn large_packets_keep_frames_and_navigation_fast() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let count = std::env::var("ICY_MAIL_PERF_COUNT")
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(200_000);
    let (_dir, mut mail) = huge(&context, count);
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    let budget = Duration::from_millis(if cfg!(debug_assertions) { 60 } else { 10 });
    let idle = timed("idle frame", 20, |_| {
        frame(&context, &mut mail, size, vec![]);
    });
    let down = timed("arrow down", 20, |_| {
        frame(&context, &mut mail, size, vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)]);
        mail.poll(&context);
    });
    let toggle = timed("toggle read", 10, |_| {
        frame(&context, &mut mail, size, vec![key(egui::Key::M, egui::Modifiers::NONE)]);
    });
    assert!(idle < budget, "idle frame took {idle:?}");
    assert!(down < budget, "navigation frame took {down:?}");
    assert!(toggle < budget, "toggling a read mark took {toggle:?}");
    let before = mail.counts.unread;
    mail.mark_folder_read(&context);
    assert_eq!(mail.counts.unread, 0, "{before} unread messages marked");
    let read = timed("show after mark all", 5, |_| {
        frame(&context, &mut mail, size, vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)]);
        wait(&mut mail, &context);
        mail.poll(&context);
    });
    assert!(read < budget * 3, "reading after marking everything took {read:?}");
    let search = timed("search keystroke", 3, |step| {
        mail.reader.filter = format!("topic {step}");
        mail.filter_changed();
        frame(&context, &mut mail, size, vec![]);
    });
    assert!(search < budget * 10, "search took {search:?}");
    let searching = timed("frame during body search", 20, |_| {
        frame(&context, &mut mail, size, vec![]);
    });
    assert!(searching < budget, "frames during body search took {searching:?}");
    mail.reader.filter.clear();
    mail.filter_changed();
    assert!(!mail.loader.searching, "clearing the query cancels the body search");
    let folders = [app::Folder::Conference(2), app::Folder::All];
    let switch = timed("switch folder", 4, |frame| mail.select_folder(folders[frame % 2]));
    let sort = timed("sort by subject", 2, |_| mail.reader.sort_messages(MessageColumn::Subject));
    let threads = timed("thread view", 2, |frame| {
        mail.set_mode(if frame % 2 == 0 { ViewMode::Threads } else { ViewMode::List })
    });
    for (name, time) in [("folder switch", switch), ("sort", sort), ("thread view", threads)] {
        assert!(time < budget * 20, "{name} took {time:?}");
    }
    mail.set_mode(ViewMode::Threads);
    let idle = timed("idle frame (threads)", 10, |_| {
        frame(&context, &mut mail, size, vec![]);
    });
    assert!(idle < budget, "idle thread frame took {idle:?}");
}

struct Gpu {
    context: egui::Context,
    device: eframe::wgpu::Device,
    queue: eframe::wgpu::Queue,
    renderer: egui_wgpu::Renderer,
}

impl Gpu {
    async fn new() -> Self {
        use eframe::wgpu;
        let adapter = wgpu::Instance::default().request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();
        let mut renderer = egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, Default::default());
        renderer
            .callback_resources
            .insert(TerminalShaderRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm));
        let context = egui::Context::default();
        appearance::apply(&context);
        Self {
            context,
            device,
            queue,
            renderer,
        }
    }

    fn capture(&mut self, mail: &mut app::MailApp, size: [u32; 2], scale: f32, events: Vec<egui::Event>, name: &str) -> (Vec<u8>, egui::FullOutput) {
        use eframe::wgpu;
        let time = self.context.input(|input| input.time) + 0.05;
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. } | egui::Event::PointerButton { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
            )),
            time: Some(time),
            events,
            modifiers,
            ..Default::default()
        };
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(scale);
        let output = self.context.run(input, |context| mail.show(context));
        let jobs = self.context.tessellate(output.shapes.clone(), output.pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            self.renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        let descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: scale,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mail capture"),
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
        let (sender, receiver) = std::sync::mpsc::channel();
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
        (pixels, output)
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_message_body_search_highlights() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    // The classic terminal view is what this test is about.
    mail.reading_mode = icy_mail::options::ReadingMode::Classic;
    mail.reader.filter = "line 1".into();
    mail.filter_changed();
    for (scale, theme) in [(1.0, egui::Theme::Dark), (2.0, egui::Theme::Light)] {
        gpu.context.set_theme(theme);
        let size = [(1100.0 * scale) as u32, (760.0 * scale) as u32];
        for _ in 0..2 {
            gpu.capture(&mut mail, size, scale, vec![], "body-search-warmup");
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while mail.loader.searching || mail.body_loading {
            assert!(Instant::now() < deadline, "body search timed out");
            gpu.capture(&mut mail, size, scale, vec![], "body-search-loading");
        }
        assert!(mail.error.is_none(), "{:?}", mail.error);
        let (pixels, _) = gpu.capture(&mut mail, size, scale, vec![], "body-search-highlighted");
        let highlighted_pixels = |pixels: &[u8], rect: egui::Rect| {
            let mut count = 0;
            for y in (rect.min.y * scale) as usize..(rect.max.y * scale) as usize {
                for x in (rect.min.x * scale) as usize..(rect.max.x * scale) as usize {
                    let offset = (y * size[0] as usize + x) * 4;
                    let [r, g, b] = [pixels[offset], pixels[offset + 1], pixels[offset + 2]];
                    count += usize::from(r > 140 && g > 100 && b < 120 && r > g && u16::from(g) > 2 * u16::from(b));
                }
            }
            count
        };
        assert!(
            highlighted_pixels(&pixels, mail.content_rect) > 100,
            "body matches must be visibly highlighted at scale {scale}"
        );
        mail.reader.filter.clear();
        mail.filter_changed();
        gpu.capture(&mut mail, size, scale, vec![], "body-search-clearing");
        let (pixels, _) = gpu.capture(&mut mail, size, scale, vec![], "body-search-cleared");
        assert_eq!(
            highlighted_pixels(&pixels, mail.content_rect),
            0,
            "clearing the filter restores the terminal colors"
        );
        mail.reader.filter = "line 1".into();
        mail.filter_changed();
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_mail_layout_themes_narrow_and_hidpi() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    // The classic terminal view is what this test is about.
    mail.reading_mode = icy_mail::options::ReadingMode::Classic;
    let package = Arc::make_mut(mail.reader.package.as_mut().unwrap());
    let other = package.infos.iter().position(|info| Some(info.index) != mail.reader.selected_message).unwrap();
    package.infos[other].from = b"Andr\x82 \xb1\xdb".as_slice().into();
    package.infos[other].subject = b"\xb0\xb1\xb2\xdb CP437 \xda\xc4\xbf".as_slice().into();
    mail.reader.rebuild_messages();
    for (size, scale, theme, name) in [
        ([1100, 760], 1.0, egui::Theme::Dark, "desktop-dark"),
        ([1100, 760], 1.0, egui::Theme::Light, "desktop-light"),
        ([360, 640], 1.0, egui::Theme::Dark, "narrow"),
        ([360, 240], 1.0, egui::Theme::Dark, "short"),
        ([1600, 1200], 2.0, egui::Theme::Light, "hidpi"),
    ] {
        gpu.context.set_theme(theme);
        for pane in [Pane::Conferences, Pane::Messages, Pane::Content] {
            mail.focus = pane;
            for _ in 0..3 {
                gpu.capture(&mut mail, size, scale, vec![], "warmup");
            }
            let (pixels, output) = gpu.capture(&mut mail, size, scale, vec![], &format!("{name}-{pane:?}"));
            label(&output, "Search messages");
            if pane == Pane::Content {
                let rect = mail.content_rect;
                assert!(rect.is_positive());
                assert!(
                    gpu.context.content_rect().contains_rect(rect),
                    "{name}: content {rect:?} outside {:?}",
                    gpu.context.content_rect()
                );
                let mut colors = std::collections::HashSet::new();
                for row in ((rect.top() * scale) as usize)..((rect.bottom() * scale) as usize).min(size[1] as usize) {
                    for column in ((rect.left() * scale) as usize)..((rect.right() * scale) as usize).min(size[0] as usize) {
                        let pos = (row * size[0] as usize + column) * 4;
                        colors.insert(pixels[pos..pos + 3].to_vec());
                    }
                }
                assert!(colors.len() >= 2, "nonblank terminal in {name}: {} colors", colors.len());
            }
        }
    }
    gpu.context.set_theme(egui::Theme::Dark);
    mail.focus = Pane::Messages;
    mail.set_mode(ViewMode::Threads);
    let starred = mail.reader.messages[0].index;
    mail.set_starred(&gpu.context.clone(), starred, true);
    for row in &mut mail.reader.conferences {
        if let Some(number) = row.number {
            row.name = format!("fsx.{}", if number == 1 { "General" } else { "Retro Computing" });
        }
    }
    for (number, name, count) in [
        (20, "fsx.BBS Support/Dev", 3),
        (21, "DOVE.Advertisements", 1),
        (22, "DOVE.General", 4),
        (23, "DOVE.Debate", 2),
        (24, "tqw.BBS Ads", 8),
        (25, "tqw.Linux", 1),
        (26, "Local Chat", 5),
    ] {
        mail.reader.conferences.push(icy_mail::reader::ConferenceRow {
            number: Some(number),
            name: name.into(),
            count,
        });
    }
    for _ in 0..3 {
        gpu.capture(&mut mail, [1600, 900], 1.0, vec![], "warmup");
    }
    gpu.capture(&mut mail, [1600, 900], 1.0, vec![], "wide-threads");
    assert!(mail.content_rect.top() < 200.0, "wide windows show the message beside the list");
    mail.set_starred(&gpu.context.clone(), starred, false);
    mail.reader.rebuild_conferences();
    mail.set_mode(ViewMode::List);
    mail.reader.filter = "coffee".into();
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "search-highlight");
    mail.reader.filter.clear();
    gpu.capture(
        &mut mail,
        [1100, 760],
        1.0,
        vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)],
        "settings-start",
    );
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "settings");
    gpu.capture(
        &mut mail,
        [1100, 760],
        1.0,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        "settings-close",
    );
    assert!(mail.settings_dialog.is_none());
    gpu.context.set_theme(egui::Theme::Dark);
    mail.focus = Pane::Messages;
    std::fs::write(
        _dir.path().join("taglines.txt"),
        "Stay a while... stay forever!\nI'd rather be downloading.\nFidoNet: where no packet has gone before.\nANSI art is not a crime.\n",
    )
    .unwrap();
    std::fs::write(
        _dir.path().join("addressbook.txt"),
        "alice\n\n\nSysop\n1:234/5\n\nWalter White\nIwalt@example.com\n\n",
    )
    .unwrap();
    let mut taglines = mail.tagline_file().unwrap();
    taglines.lines.truncate(1);
    mail.taglines = Some(taglines);
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::R, egui::Modifiers::COMMAND)], "compose-start");
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    let (_, output) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "compose");
    label(&output, "Save Draft");
    let steps: Vec<(Vec<egui::Event>, &str)> = vec![
        (
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE), key(egui::Key::Enter, egui::Modifiers::NONE)],
            "",
        ),
        (vec![key(egui::Key::Escape, egui::Modifiers::NONE), text("\nSounds great! ")], ""),
        (
            vec![
                key(egui::Key::K, egui::Modifiers::COMMAND),
                text("b"),
                key(egui::Key::ArrowUp, egui::Modifiers::NONE),
            ],
            "editor-colors",
        ),
        (vec![key(egui::Key::Enter, egui::Modifiers::NONE), text("Colors work too.")], "editor-text"),
        (
            vec![key(egui::Key::G, egui::Modifiers::COMMAND), key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            "editor-chars",
        ),
        (
            vec![key(egui::Key::Enter, egui::Modifiers::NONE), key(egui::Key::Q, egui::Modifiers::COMMAND)],
            "editor-quotes",
        ),
        (
            vec![key(egui::Key::Escape, egui::Modifiers::NONE), key(egui::Key::F, egui::Modifiers::COMMAND)],
            "",
        ),
        (vec![text("great"), key(egui::Key::Enter, egui::Modifiers::NONE)], "editor-find"),
        (
            vec![key(egui::Key::Escape, egui::Modifiers::NONE), key(egui::Key::Escape, egui::Modifiers::NONE)],
            "",
        ),
        (
            vec![key(egui::Key::Escape, egui::Modifiers::NONE), key(egui::Key::F1, egui::Modifiers::NONE)],
            "editor-help",
        ),
        (vec![key(egui::Key::Escape, egui::Modifiers::NONE)], ""),
        (
            vec![key(egui::Key::T, egui::Modifiers::COMMAND), key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            "taglines-pick",
        ),
        (
            vec![key(egui::Key::Enter, egui::Modifiers::NONE), key(egui::Key::B, egui::Modifiers::COMMAND)],
            "address-pick",
        ),
        (vec![key(egui::Key::Escape, egui::Modifiers::NONE)], "compose-tagline"),
    ];
    for (events, name) in steps {
        for event in events {
            gpu.capture(&mut mail, [1100, 760], 1.0, vec![event], "warmup");
        }
        if !name.is_empty() {
            gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
            gpu.capture(&mut mail, [1100, 760], 1.0, vec![], name);
        }
    }
    assert!(mail.modal.is_none());
    let composer = mail.composer.as_ref().unwrap();
    assert!(
        composer.draft.body.contains("Sounds great!") && composer.draft.body.contains('\x1b'),
        "{:?}",
        composer.draft.body
    );
    assert_eq!(composer.draft.tagline, "I'd rather be downloading.");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::S, egui::Modifiers::COMMAND)], "compose-save");
    mail.select_folder(app::Folder::Drafts);
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    let (_, output) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "outbox");
    label(&output, "Re: Coffee machine");
    let mut welcome = app::MailApp::with_storage(&gpu.context, _dir.path().to_path_buf());
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        gpu.context.set_theme(theme);
        for _ in 0..3 {
            gpu.capture(&mut welcome, [1100, 760], 1.0, vec![], "warmup");
        }
        let (_, output) = gpu.capture(&mut welcome, [1100, 760], 1.0, vec![], &format!("welcome-{theme:?}"));
        label(&output, "TEST BBS");
    }
    gpu.context.set_theme(egui::Theme::Dark);
    let (_, output) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    let menu = egui::pos2(1100.0 - 8.0 - widgets::TOOL_SIZE.x / 2.0, label(&output, "Search messages").center().y);
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(menu, true), "warmup");
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(menu, false), "warmup");
    let (_, output) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "menu");
    let (_, output) = gpu.capture(
        &mut mail,
        [1100, 760],
        1.0,
        vec![egui::Event::PointerMoved(label(&output, "Message").center())],
        "warmup",
    );
    let message = label(&output, "Message").center();
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(message, true), "warmup");
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(message, false), "warmup");
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    let (_, output) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "menu-message");
    label(&output, "Next Unread");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::Escape, egui::Modifiers::NONE)], "warmup");
    welcome.open_about();
    for _ in 0..3 {
        gpu.capture(&mut welcome, [1100, 760], 1.0, vec![], "warmup");
    }
    gpu.capture(&mut welcome, [1100, 760], 1.0, vec![], "about");
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_long_message_scroll_and_selection_copy() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    // The classic terminal view is what this test is about.
    mail.reading_mode = icy_mail::options::ReadingMode::Classic;
    let body = format!("\x1b[31mHELLO WORLD\x1b[0m\n{}THE END", "more text\n".repeat(200));
    mail.screen = icy_engine_gui::egui::screen::ScreenView::new(icy_mail::reader::render_body(body.as_bytes()).unwrap());
    mail.focus = Pane::Content;
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "long-warmup");
    }
    assert!(mail.screen.max_offset.y > 1000.0);
    let (start, end) = {
        let info = mail.screen.terminal.render_info.read();
        let start = egui::pos2(
            info.bounds_x + info.viewport_x + info.font_width * info.display_scale * 0.1,
            info.bounds_y + info.viewport_y + info.font_height * info.display_scale * 0.5,
        );
        (start, start + egui::vec2(info.font_width * info.display_scale * 4.8, 0.0))
    };
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(start, true), "selection-start");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![egui::Event::PointerMoved(end)], "selection-drag");
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(end, false), "selection-end");
    let (_, copied) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![egui::Event::Copy], "selection-copy");
    assert!(
        copied
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text == "HELLO")),
        "{:?}",
        copied.platform_output.commands
    );
    gpu.capture(
        &mut mail,
        [1100, 760],
        1.0,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        "selection-escape",
    );
    assert!(mail.screen.terminal.screen.lock().selection().is_none());
    let word = start + egui::vec2((end.x - start.x) * 0.5, 0.0);
    for _ in 0..2 {
        gpu.capture(&mut mail, [1100, 760], 1.0, pointer(word, true), "word-press");
        gpu.capture(&mut mail, [1100, 760], 1.0, pointer(word, false), "word-release");
    }
    assert_eq!(mail.screen.terminal.screen.lock().copy_text().as_deref(), Some("HELLO"));
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(word, true), "line-press");
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(word, false), "line-release");
    {
        let screen = mail.screen.terminal.screen.lock();
        let selection = screen.selection().unwrap();
        assert_eq!(selection.anchor, (0, 0).into());
        assert_eq!(selection.lead, (screen.width() - 1, 0).into());
    }
    let shifted = |position, pressed, modifiers| {
        let mut events = pointer(position, pressed);
        if let egui::Event::PointerButton {
            modifiers: event_modifiers, ..
        } = &mut events[1]
        {
            *event_modifiers = modifiers;
        }
        events
    };
    let second_row = {
        let info = mail.screen.terminal.render_info.read();
        end + egui::vec2(0.0, info.font_height * info.display_scale)
    };
    gpu.capture(&mut mail, [1100, 760], 1.0, shifted(second_row, true, egui::Modifiers::SHIFT), "extend-press");
    gpu.capture(
        &mut mail,
        [1100, 760],
        1.0,
        shifted(second_row, false, egui::Modifiers::SHIFT),
        "extend-release",
    );
    {
        let selection = mail.screen.terminal.screen.lock().selection().unwrap();
        assert_eq!(selection.anchor, (0, 0).into());
        assert_eq!(selection.lead, (4, 1).into());
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, shifted(start, true, egui::Modifiers::ALT), "rectangle-press");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![egui::Event::PointerMoved(second_row)], "rectangle-drag");
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(second_row, false), "rectangle-release");
    assert_eq!(mail.screen.terminal.screen.lock().selection().unwrap().shape, icy_engine::Shape::Rectangle);
    assert!(mail.selection_anchor.is_none());
    for point in [word, start, second_row] {
        gpu.capture(&mut mail, [1100, 760], 1.0, pointer(point, true), "single-press");
        assert!(mail.screen.terminal.screen.lock().selection().is_none(), "clear on mouse-down");
        gpu.capture(&mut mail, [1100, 760], 1.0, pointer(point, false), "single-release");
        assert!(
            mail.screen.terminal.screen.lock().selection().is_none(),
            "separate clicks must not create word/line selections"
        );
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::End, egui::Modifiers::NONE)], "long-end");
    assert!(mail.screen.offset.y > 1000.0);
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::Home, egui::Modifiers::NONE)], "long-home");
    assert_eq!(mail.screen.offset.y, 0.0);
    let pointer = mail.content_rect.center();
    gpu.capture(
        &mut mail,
        [1100, 760],
        1.0,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -300.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        "long-wheel",
    );
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "long-wheel-settled");
    }
    assert!(mail.screen.offset.y > 0.0);
}

#[test]
fn ui_font_covers_every_cp437_character() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let _ = context.run(Default::default(), |_| {});
    let missing: String = (128u8..=255)
        .map(icy_mail::editor::cp437_char)
        .filter(|&ch| !context.fonts_mut(|fonts| fonts.has_glyph(&egui::FontId::proportional(13.0), ch)))
        .collect();
    assert!(missing.is_empty(), "missing glyphs: {missing}");
}

#[test]
#[ignore = "temp"]
fn zz_temp_editor_rows() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::N, egui::Modifiers::COMMAND)], "warmup");
    mail.composer.as_mut().unwrap().editor.request_focus();
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    for line in 0..30 {
        gpu.capture(
            &mut mail,
            [1100, 760],
            1.0,
            vec![text(&format!("line {line}")), key(egui::Key::Enter, egui::Modifiers::NONE)],
            "warmup",
        );
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::Home, egui::Modifiers::COMMAND)], "warmup");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "editor-rows");
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_reader_deselecting_clears_the_rendered_highlight() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    // The classic terminal view is what this test is about.
    mail.reading_mode = icy_mail::options::ReadingMode::Classic;
    let body = format!("HELLO WORLD\n{}THE END", "more text\n".repeat(80));
    mail.screen = icy_engine_gui::egui::screen::ScreenView::new(icy_mail::reader::render_body(body.as_bytes()).unwrap());
    mail.set_focus(Pane::Messages, &gpu.context);
    let size = [1100, 760];
    for _ in 0..3 {
        gpu.capture(&mut mail, size, 1.0, vec![], "warmup");
    }
    let rect = mail.content_rect;
    // Only the message body, so hover effects elsewhere do not count as changes.
    let body_pixels = move |pixels: &[u8]| -> Vec<u8> {
        let width = size[0] as usize;
        (rect.top() as usize..(rect.bottom() as usize).min(size[1] as usize))
            .flat_map(|y| pixels[(y * width + rect.left() as usize) * 4..(y * width + (rect.right() as usize).min(width)) * 4].to_vec())
            .collect()
    };
    let (start, end) = {
        let info = mail.screen.terminal.render_info.read();
        let start = egui::pos2(
            info.bounds_x + info.viewport_x + info.font_width * info.display_scale * 0.5,
            info.bounds_y + info.viewport_y + info.font_height * info.display_scale * 0.5,
        );
        (
            start,
            start + egui::vec2(info.font_width * info.display_scale * 4.0, info.font_height * info.display_scale * 2.0),
        )
    };
    let (baseline, _) = gpu.capture(&mut mail, size, 1.0, vec![egui::Event::PointerMoved(start)], "hover");
    let baseline = body_pixels(&baseline);
    let select = |gpu: &mut Gpu, mail: &mut app::MailApp| {
        gpu.capture(mail, size, 1.0, pointer(start, true), "press");
        for step in 1..=4 {
            gpu.capture(
                mail,
                size,
                1.0,
                vec![egui::Event::PointerMoved(start + (end - start) * (step as f32 / 4.0))],
                "drag",
            );
        }
        gpu.capture(mail, size, 1.0, pointer(end, false), "release").0
    };
    let selected = select(&mut gpu, &mut mail);
    assert_ne!(body_pixels(&selected), baseline, "the drag must be highlighted");
    {
        let selection = mail.screen.terminal.screen.lock().selection().expect("drag selects");
        assert_eq!((selection.anchor, selection.lead), ((0, 0).into(), (4, 2).into()));
    }
    assert_eq!(mail.focus, Pane::Content, "dragging in the body must focus it");

    // Every tile is served from the cache again once the selection is gone,
    // which used to leave the old highlight on the GPU.
    let (escaped, _) = gpu.capture(&mut mail, size, 1.0, vec![key(egui::Key::Escape, egui::Modifiers::NONE)], "escape");
    assert!(mail.screen.terminal.screen.lock().selection().is_none(), "Escape must deselect");
    assert!(body_pixels(&escaped) == baseline, "Escape must remove the highlight from the screen");

    select(&mut gpu, &mut mail);
    std::thread::sleep(std::time::Duration::from_millis(600));
    gpu.capture(&mut mail, size, 1.0, pointer(start, true), "click-press");
    let (clicked, _) = gpu.capture(&mut mail, size, 1.0, pointer(start, false), "click-release");
    assert!(mail.screen.terminal.screen.lock().selection().is_none(), "a click must deselect");
    assert!(body_pixels(&clicked) == baseline, "a click must remove the highlight from the screen");
}

#[test]
fn list_dates_are_relative_for_the_past_week() {
    crate::use_english();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
    let at = |day: u32, time: &str| {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 9, day)
            .unwrap()
            .and_time(chrono::NaiveTime::parse_from_str(time, "%H:%M").unwrap());
        list::friendly_date(date, &date.format("%Y-%m-%d %H:%M").to_string(), today)
    };
    assert_eq!(at(29, "08:00"), "Today 08:00");
    assert_eq!(at(28, "23:59"), "Yesterday 23:59");
    assert_eq!(at(27, "10:15"), "Sunday 10:15");
    assert_eq!(at(23, "07:00"), "Wednesday 07:00");
    assert_eq!(at(22, "07:00"), "2026-09-22");
    assert_eq!(at(30, "07:00"), "2026-09-30", "future dates stay absolute");
    let unparsed = chrono::NaiveDateTime::default();
    assert_eq!(list::friendly_date(unparsed, "13-45-2612:00", today), "13-45-2612:00");
    assert_eq!(list::friendly_qwk_date("09-28-2608:07", today), "Yesterday 08:07");
}

#[test]
fn thread_replies_dim_the_subject_they_repeat() {
    let dim = |subject: &str, parent: &str| subject[..list::repeated_subject_len(subject, parent)].to_string();
    assert_eq!(dim("Re: Amiga demos", "Amiga demos"), "Re: Amiga demos");
    assert_eq!(dim("RE[2]: amiga DEMOS ", "Re: Amiga demos"), "RE[2]: amiga DEMOS ");
    assert_eq!(
        dim("FidoNews 43:39 [01/06]: Jamnntpd Servers List", "FidoNews 43:39 [00/06]: The Front Page"),
        "FidoNews 43:39 "
    );
    assert_eq!(dim("Re: Amiga games", "Amiga demos"), "Re: Amiga ");
    assert_eq!(dim("Re: A new topic", "A question"), "Re: ", "short shared words stay visible");
    assert_eq!(dim("Something else", "Amiga demos"), "");
    assert_eq!(dim("Grüße aus Köln", "Grüße aus Bonn"), "Grüße aus ");
}

#[test]
fn reading_pane_moves_beside_the_list_on_wide_windows() {
    use icy_mail::options::ReadingPane;
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let beside = |mail: &mut app::MailApp, size: egui::Vec2| {
        settle(&context, mail, size);
        let list = mail.content_rect;
        list.top() < 200.0
    };
    let wide = egui::vec2(1600.0, 900.0);
    let desktop = egui::vec2(1100.0, 760.0);
    assert!(beside(&mut mail, wide), "automatic layout uses the width");
    let output = settle(&context, &mut mail, wide);
    assert!(
        label(&output, "Subject").right() < mail.content_rect.left(),
        "list columns sit left of the message"
    );
    assert!(!beside(&mut mail, desktop), "automatic layout stacks on smaller windows");
    mail.reading_pane = ReadingPane::Right;
    assert!(beside(&mut mail, egui::vec2(1300.0, 800.0)), "right is honored while both fit");
    assert!(!beside(&mut mail, desktop), "too narrow for side by side");
    mail.reading_pane = ReadingPane::Below;
    assert!(!beside(&mut mail, wide));
}

#[test]
fn stars_keep_messages_for_later_and_survive_reopening() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    settle(&context, &mut mail, size);
    let first = mail.reader.selected_message.unwrap();
    click_label(&context, &mut mail, size, "Starred");
    let output = settle(&context, &mut mail, size);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().starts_with("Star messages with S"))),
        "the empty starred mailbox explains how to fill it"
    );
    click_label(&context, &mut mail, size, "All Messages");
    mail.reader.select_message(first);
    mail.set_focus(Pane::Messages, &context);
    frame(&context, &mut mail, size, vec![key(egui::Key::S, egui::Modifiers::NONE)]);
    assert!(mail.reader.is_starred(first));
    assert_eq!(mail.counts.starred, 1);

    // The star beside another row toggles it without moving the selection.
    let output = settle(&context, &mut mail, size);
    let carol = label(&output, "carol");
    let star = egui::pos2(carol.left() - 47.0, carol.center().y);
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(star, pressed));
    }
    let package = mail.reader.package.clone().unwrap();
    let carol = package.infos.iter().position(|info| info.from.as_str() == "carol").unwrap();
    assert!(mail.reader.is_starred(carol), "clicking the row's star stars it");
    assert_eq!(mail.reader.selected_message, Some(first), "starring from the list keeps the selection");
    assert_eq!(mail.counts.starred, 2);

    click_label(&context, &mut mail, size, "Starred");
    assert_eq!(mail.folder, app::Folder::Starred);
    let mut rows: Vec<_> = mail.reader.messages.iter().map(|row| row.index).collect();
    rows.sort_unstable();
    let mut expected = vec![first, carol];
    expected.sort_unstable();
    assert_eq!(rows, expected);
    mail.reader.select_message(carol);
    frame(&context, &mut mail, size, vec![key(egui::Key::S, egui::Modifiers::NONE)]);
    assert!(!mail.reader.is_starred(carol));
    assert_eq!(mail.reader.messages.len(), 2, "an unstarred message stays until the folder is reopened");

    let mut reopened = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    reopened.open(dir.path().join("TEST.QWK"), &context);
    wait(&mut reopened, &context);
    assert!(reopened.reader.is_starred(first) && !reopened.reader.is_starred(carol));
    assert_eq!(reopened.counts.starred, 1);
}

#[test]
fn dragging_a_file_over_the_start_page_highlights_the_drop_zone() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, _mail) = loaded(&context);
    let mut mail = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    let size = egui::vec2(1100.0, 760.0);
    let output = settle(&context, &mut mail, size);
    assert_eq!(count(&output, "Release to open the packet"), 0);
    let output = context.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            hovered_files: vec![egui::HoveredFile {
                path: Some(dir.path().join("TEST.QWK")),
                ..Default::default()
            }],
            ..Default::default()
        },
        |context| mail.show(context),
    );
    label(&output, "Release to open the packet");
}

#[test]
fn main_menu_groups_actions_and_opens_a_new_window() {
    let context = egui::Context::default();
    context.set_embed_viewports(false);
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    let output = settle(&context, &mut mail, size);
    let toolbar = label(&output, "Search messages").center().y;
    let menu = egui::pos2(size.x - 8.0 - widgets::TOOL_SIZE.x / 2.0, toolbar);
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(menu, pressed));
    }
    let output = settle(&context, &mut mail, size);
    for title in ["File", "Message", "View", "Tools", "Help"] {
        label(&output, title);
    }
    for hidden in ["New Window", "Reply", "Address Book\u{2026}", "About Icy Mail"] {
        assert_eq!(count(&output, hidden), 0, "{hidden} belongs in a submenu");
    }
    let file = label(&output, "File").center();
    frame(&context, &mut mail, size, vec![egui::Event::PointerMoved(file)]);
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(file, pressed));
    }
    let output = settle(&context, &mut mail, size);
    let new_window = label(&output, "New Window");
    assert!(
        new_window.top() <= label(&output, "Open Packet\u{2026}").top(),
        "New Window leads the File menu"
    );
    frame(&context, &mut mail, size, vec![egui::Event::PointerMoved(new_window.center())]);
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(new_window.center(), pressed));
    }
    let output = frame(&context, &mut mail, size, vec![]);
    assert_eq!(output.viewport_output.len(), 2, "a second window was opened");
}

#[test]
fn conference_names_split_into_network_and_area() {
    use sidebar::network_of;
    assert_eq!(network_of("fsx.Chat, Testing + More"), Some(("fsx", "Chat, Testing + More")));
    assert_eq!(network_of("DOVE.General"), Some(("DOVE", "General")));
    assert_eq!(network_of("Mr. Smith's Area"), None, "a space after the dot is a sentence");
    assert_eq!(network_of("Release v1.2 notes"), None);
    assert_eq!(network_of("v1.2"), None, "version numbers are no network");
    assert_eq!(network_of("General"), None);
    assert_eq!(network_of(".hidden"), None);
}

#[test]
fn conferences_group_by_network_in_sidebar_order() {
    use sidebar::{conference_tree, ConferenceNode};
    let rows = |names: &[&str]| {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| (index as u16 + 1, name.to_string(), 10))
            .collect::<Vec<_>>()
    };
    let tree = conference_tree(&rows(&["fsx.Chat", "Local", "DOVE.General", "fsx.BBS Ads", "dove.Debate", "tqw.Linux"]));
    assert_eq!(
        tree,
        vec![
            ConferenceNode::Group {
                network: "fsx".into(),
                members: vec![(1, "Chat".into(), 10), (4, "BBS Ads".into(), 10)],
            },
            ConferenceNode::Single {
                number: 2,
                name: "Local".into(),
                count: 10
            },
            ConferenceNode::Group {
                network: "DOVE".into(),
                members: vec![(3, "General".into(), 10), (5, "Debate".into(), 10)],
            },
            ConferenceNode::Single {
                number: 6,
                name: "tqw.Linux".into(),
                count: 10
            },
        ],
        "networks need two conferences and match case-insensitively"
    );
    let single_network = conference_tree(&rows(&["fsx.Chat", "fsx.Ads"]));
    assert!(
        single_network.iter().all(|node| matches!(node, ConferenceNode::Single { .. })),
        "one network holding everything stays flat"
    );
}

#[test]
fn network_groups_collapse_filter_and_open_for_next_unread() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    for row in &mut mail.reader.conferences {
        if let Some(number) = row.number {
            row.name = format!("net.{}", if number == 1 { "General" } else { "Retro" });
        }
    }
    // First in the sidebar, so the next conference with unread mail lies in the network.
    mail.reader.conferences.insert(
        1,
        icy_mail::reader::ConferenceRow {
            number: Some(9),
            name: "Local".into(),
            count: 0,
        },
    );
    let output = settle(&context, &mut mail, size);
    let header = label(&output, "net");
    let general = label(&output, "General");
    assert!(general.left() > header.left(), "members are indented under the network");
    label(&output, "Local");
    assert_eq!(mail.folders().iter().filter(|folder| matches!(folder, app::Folder::Conference(_))).count(), 3);

    click_label(&context, &mut mail, size, "net");
    let output = settle(&context, &mut mail, size);
    assert_eq!(count(&output, "General"), 0, "a closed network hides its conferences");
    assert!(!mail.folders().contains(&app::Folder::Conference(1)), "arrow keys skip hidden conferences");
    assert!(mail.all_folders().contains(&app::Folder::Conference(1)));

    // Next Unread still walks into the closed network and opens it.
    click_label(&context, &mut mail, size, "Local");
    assert_eq!(mail.folder, app::Folder::Conference(9));
    mail.next_unread(&context);
    assert!(matches!(mail.folder, app::Folder::Conference(1 | 2)), "{:?}", mail.folder);
    let output = settle(&context, &mut mail, size);
    label(&output, "General");

    mail.conferences_unread_only = true;
    let output = settle(&context, &mut mail, size);
    assert_eq!(count(&output, "Local"), 0, "conferences without unread messages are hidden");
    assert!(mail.current_options(&context).conferences_unread_only, "the filter is saved");
}

#[test]
fn modern_reading_mode_sets_lines_by_content() {
    use modern_view::{blocks, is_quote, readable, Block};
    let render = |data: &[u8]| (icy_mail::reader::render_body(data).unwrap(), icy_mail::reader::render_body_wide(data).unwrap());
    let (classic, wide) = render(b"Hello there,\r\n JD> quoted text\r\n> > nested\r\nName    Size    Date\r\n\xda\xc4\xc4\xbf\r\n\x1b[1;34mblue\x1b[0m\r\n");
    let lines: Vec<_> = blocks(&classic, &wide)
        .into_iter()
        .map(|block| match block {
            Block::Text(line) => line,
            Block::Art { .. } => panic!("no art in plain text"),
        })
        .collect();
    let flags: Vec<(bool, bool)> = lines.iter().map(|line| (line.fixed, line.quote)).collect();
    assert_eq!(
        flags,
        [(false, false), (false, true), (false, true), (true, false), (true, false), (false, false)],
        "{lines:?}"
    );
    assert!(!is_quote("Hello > world"), "a marker after words is no quote");
    assert!(!is_quote("Longname> text"));
    let dark_blue = readable([0, 0, 170], false, true);
    assert!(dark_blue.r() > 100, "dark colors are lightened on dark themes: {dark_blue:?}");
    let yellow = readable([255, 255, 85], false, false);
    assert!(yellow.g() < 180, "light colors are darkened on light themes: {yellow:?}");
    assert_eq!(
        readable([0, 0, 170], true, true),
        egui::Color32::from_rgb(0, 0, 170),
        "colors on their own background stay"
    );

    // Text stays whole; block graphics and colored backgrounds become art, including short gaps.
    let data = [
        b"word ".repeat(30),
        b"\r\n\r\n  \xdc\xdf\xdb\xb0\r\n\r\n \x1b[44m    \x1b[0m\r\ntext\r\n\xdb\r\n\r\n\r\n\r\n\xdb\r\n".to_vec(),
    ]
    .concat();
    let describe = |(classic, wide): (icy_engine::TextScreen, icy_engine::TextScreen)| -> Vec<String> {
        blocks(&classic, &wide)
            .into_iter()
            .map(|block| match block {
                Block::Text(line) => format!("text {}", line.spans.iter().map(|span| span.text.as_str()).collect::<String>()),
                Block::Art { rows, columns } => format!("art {rows:?} {columns}"),
            })
            .collect()
    };
    let parts = describe(render(&data));
    assert_eq!(
        parts[0],
        format!("text {}", "word ".repeat(30).trim_end()),
        "wrapped text is joined, keeping the space"
    );
    assert_eq!(parts[1..], ["text ", "art 3..12 6"], "paragraphs with art and short gaps form one picture");

    // Bullets like ■ in a tagline are text, not art.
    assert_eq!(
        describe(render(b"---\r\n \xfe SLMR Rob  \xfe It costs $1.25 to mint a penny \xfe\r\n")),
        ["text ---", "text  \u{25a0} SLMR Rob  \u{25a0} It costs $1.25 to mint a penny \u{25a0}"]
    );

    // Colored lines between two pieces of art belong to the picture; a plain paragraph ends it.
    let ad = b"\xdb\xdb\r\n\r\n\x1b[36mtelnet>>bbs.example.com\x1b[0m\r\n\r\n\xdf\xdf\r\n\r\nJust some words.\r\n\r\n\xdc\xdc\r\n";
    assert_eq!(describe(render(ad)), ["art 0..5 23", "text ", "text Just some words.", "text ", "art 8..9 2"]);

    // Art filling whole 80 column rows without line breaks relies on the terminal wrapping.
    let art = [vec![0xdb; 80], vec![0xb0; 80], b"\r\n\x1b[36mfsxNet 21:2/150\x1b[0m\r\n".to_vec()].concat();
    assert_eq!(
        describe(render(&art)),
        ["art 0..2 80", "text ", "text fsxNet 21:2/150"],
        "the terminal's empty row stays"
    );
    let exact = [b"x".repeat(80), b"\r\n".to_vec(), b"word ".repeat(17), b"end\r\n".to_vec()].concat();
    assert_eq!(
        describe(render(&exact)),
        [format!("text {}", "x".repeat(80)), "text ".into(), format!("text {}end", "word ".repeat(17))],
        "a full row does not stop later lines from being joined"
    );
}

#[test]
fn modern_reading_mode_shows_selectable_text_and_keeps_keyboard_reading() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    mail.reading_mode = icy_mail::options::ReadingMode::Modern;
    let output = settle(&context, &mut mail, size);
    let body = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with("line 0\nline 1\n") => Some(text.galley.rect.translate(text.pos.to_vec2())),
            _ => None,
        })
        .expect("the message is drawn as text");
    assert!(body.left() >= mail.content_rect.left() + 20.0, "the text keeps a margin");
    assert_eq!(mail.screen.max_offset.y, 0.0, "a short message does not scroll");
    let first = mail.reader.selected_message;
    mail.set_focus(Pane::Content, &context);
    frame(&context, &mut mail, size, vec![key(egui::Key::Space, egui::Modifiers::NONE)]);
    assert_ne!(mail.reader.selected_message, first, "Space at the end continues with the next unread message");
    assert!(mail.current_options(&context).reading_mode == icy_mail::options::ReadingMode::Modern);
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_modern_reading_mode() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    let body: Vec<u8> = [
        b"Hi Alice,\r\n\r\n".as_slice(),
        b" AL> The coffee machine on the third floor is broken again.\r\n".as_slice(),
        b" AL> Does anyone know who to call?\r\n\r\n".as_slice(),
        b"I called the facility team this morning. They will fix it on \x1b[1;33mTuesday\x1b[0m, and until then\r\n".as_slice(),
        b"we can use the one in the \x1b[1;36mkitchen downstairs\x1b[0m. Here is the \x1b[1;31mupdated\x1b[0m plan:\r\n\r\n".as_slice(),
        b"\xda\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc2\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xbf\r\n".as_slice(),
        b"\xb3 Day      \xb3 Machine    \xb3\r\n".as_slice(),
        b"\xc3\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc5\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xb4\r\n".as_slice(),
        b"\xb3 Mon-Mon  \xb3 \x1b[32mkitchen\x1b[0m    \xb3\r\n".as_slice(),
        b"\xb3 Tuesday  \xb3 \x1b[32mthird floor\x1b[0m\xb3\r\n".as_slice(),
        b"\xc0\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc1\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xc4\xd9\r\n\r\n".as_slice(),
        b"Cheers,\r\nBob\r\n\r\n".as_slice(),
        b"\x1b[1;30m... \x1b[0;34mI'd rather be downloading.\x1b[0m\r\n".as_slice(),
    ]
    .concat();
    // The long line checks that text is not cut at the terminal's 80 columns.
    let body = [
        body.as_slice(),
        b"P.S. This line is longer than eighty columns, so the classic terminal would have to wrap it.\r\n\r\n",
        b"  \x1b[1;36m\xdc\xdc\xdc\xdc\xdc  \x1b[1;33m\xdc\xdc\xdc\xdc\xdc  \x1b[1;35m\xdb\xdb   \xdb\xdb\x1b[0m\r\n",
        b"  \x1b[1;36m\xdb\x1b[46m \x1b[0;36m\xb1\xb1\x1b[1;36m\x1b[40m\xdb  \x1b[1;33m\xdb\x1b[0;33m\xb2\xb2\xb2\x1b[1;33m\xdb  \x1b[1;35m\xdb\xdb\xdc \xdc\xdb\xdb  \x1b[0mIcy Mail\r\n",
        b"  \x1b[1;36m\xdf\xdf\xdf\xdf\xdf  \x1b[1;33m\xdf\xdf\xdf\xdf\xdf  \x1b[1;35m\xdf\xdf \xdf \xdf\xdf\x1b[0m\r\n",
        b"\x1b[0;37m\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\x1b[0m\r\n",
        b"\x1b[36mtelnet>>\x1b[1;36micy.example.com\x1b[0;36m:1337\x1b[0m\r\n\r\n",
        b"   \x1b[1;36mfsxNet \x1b[0;36m21:2/150      \x1b[1;36mDove-Net      \x1b[1;36mtqwNet \x1b[0;36m1337:3/129\x1b[0m\r\n\r\n",
        b"\x1b[0;37m\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\xdc\xdf\x1b[0m\r\n\r\n\r\n",
        b"--- Icy Mail\r\n * Origin: Somewhere (21:2/150)\r\n",
    ]
    .concat();
    let package = mail.reader.package.clone().unwrap();
    let cache_key = (
        Arc::as_ptr(&package) as usize,
        modern_view::Document::Message(mail.reader.selected_message.unwrap()),
    );
    let classic = icy_mail::reader::render_body(&body).unwrap();
    let wide = icy_mail::reader::render_body_wide(&body).unwrap();
    mail.modern_items = Some((cache_key, modern_view::items(&gpu.context, &classic, modern_view::blocks(&classic, &wide))));
    mail.reading_mode = icy_mail::options::ReadingMode::Modern;
    for (theme, font, name) in [
        (egui::Theme::Dark, icy_mail::options::ModernFont::Proportional, "modern-dark"),
        (egui::Theme::Light, icy_mail::options::ModernFont::Proportional, "modern-light"),
        (egui::Theme::Dark, icy_mail::options::ModernFont::Monospace, "modern-monospace"),
    ] {
        gpu.context.set_theme(theme);
        mail.modern_font = font;
        for _ in 0..3 {
            gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
        }
        let (_, output) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![], name);
        assert!(output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("\u{250c}\u{2500}"))));
    }
    gpu.context.set_theme(egui::Theme::Dark);
    mail.modern_font = icy_mail::options::ModernFont::Proportional;
    mail.screen.scroll_to = Some(egui::vec2(0.0, f32::MAX));
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "modern-end");
    mail.select_folder(app::Folder::Bulletins);
    wait(&mut mail, &gpu.context);
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "modern-bulletin");
    // The editor in the modern display: an empty new message, then a reply with quotes.
    mail.select_folder(app::Folder::All);
    for (theme, name) in [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")] {
        gpu.context.set_theme(theme);
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::N, egui::Modifiers::COMMAND)], "warmup");
        for _ in 0..3 {
            gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
        }
        let (_, output) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![], &format!("modern-compose-empty-{name}"));
        label(&output, "Write your message\u{2026} F1 shows all shortcuts.");
        mail.cancel_composer();
    }
    gpu.context.set_theme(egui::Theme::Dark);
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::R, egui::Modifiers::COMMAND)], "warmup");
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "warmup");
    }
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![egui::Event::Text("Thanks, that helps!".into())], "warmup");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "modern-compose-reply");
}

#[test]
fn messages_are_saved_as_written_or_as_utf8() {
    let context = egui::Context::default();
    let (_dir, mail) = loaded(&context);
    let package = mail.reader.package.clone().unwrap();
    let index = mail.reader.selected_message.unwrap();
    let info = &package.infos[index];
    let (name, original) = app::message_file(&package, index, false).unwrap();
    assert_eq!(name, format!("{}-{}.ans", info.conference, info.number));
    assert_eq!(original, package.get_message(index).unwrap().text.to_vec(), "the original bytes are kept");
    let (name, utf8) = app::message_file(&package, index, true).unwrap();
    assert!(name.ends_with(".txt"));
    assert!(String::from_utf8(utf8).unwrap().contains("line 0"));
}

#[test]
fn every_folder_shows_its_text_in_the_chosen_display() {
    use modern_view::Document;
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    assert_eq!(mail.reading_mode, icy_mail::options::ReadingMode::Modern);
    let shown = |mail: &app::MailApp| mail.modern_items.as_ref().map(|((_, document), _)| *document);
    let texts = |output: &egui::FullOutput| -> Vec<String> {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect()
    };

    settle(&context, &mut mail, size);
    assert!(matches!(shown(&mail), Some(Document::Message(_))));

    frame(&context, &mut mail, size, vec![key(egui::Key::R, egui::Modifiers::COMMAND)]);
    settle(&context, &mut mail, size);
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    frame(&context, &mut mail, size, vec![egui::Event::Text("Modern draft".into())]);
    frame(&context, &mut mail, size, vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    click_label(&context, &mut mail, size, "Outbox");
    let output = settle(&context, &mut mail, size);
    assert!(matches!(shown(&mail), Some(Document::Draft(..))), "the outbox preview uses the modern view");
    assert!(texts(&output).iter().any(|text| text.contains("Modern draft")), "{:?}", texts(&output));

    click_label(&context, &mut mail, size, "Bulletins");
    wait(&mut mail, &context);
    settle(&context, &mut mail, size);
    assert!(matches!(shown(&mail), Some(Document::File(0, 0))), "bulletins use the modern view");

    mail.reading_mode = icy_mail::options::ReadingMode::Classic;
    mail.modern_items = None;
    settle(&context, &mut mail, size);
    assert!(mail.modern_items.is_none(), "the classic display draws the terminal instead");
}
