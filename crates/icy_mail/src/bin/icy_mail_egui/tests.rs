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
fn new_and_forward_create_distinct_drafts() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    click_label(&context, &mut mail, size, "New");
    click_label(&context, &mut mail, size, "Save Draft");
    let draft = &mail.drafts.as_ref().unwrap().drafts()[0];
    assert_eq!(draft.to, "ALL");
    assert_eq!(draft.conference, 1);
    assert_eq!(draft.ref_number, 0);
    click_label(&context, &mut mail, size, "Forward");
    click_label(&context, &mut mail, size, "Save Draft");
    let draft = &mail.drafts.as_ref().unwrap().drafts()[1];
    assert!(draft.to.is_empty());
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
            egui::Event::Key { modifiers, .. } => Some(*modifiers),
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
            for text in ["Open", "New", "Reply", "Forward", "Export Replies"] {
                assert!(screen.contains_rect(label(&output, text)), "{size:?}: {text}");
            }
        } else {
            assert_eq!(count(&output, "Reply"), 0, "{size:?}: compact toolbar shows icons only");
        }
    }
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
    frame(&context, &mut mail, size, vec![key(egui::Key::N, egui::Modifiers::NONE)]);
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
    label(&output, "1 reply to send");
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
    assert_eq!(mail.draft_count(), 1, "incomplete drafts can be saved");
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
    click_label(&context, &mut mail, size, "TEST.QWK");
    wait(&mut mail, &context);
    assert!(mail.reader.package.is_some());
    let mut fresh = app::MailApp::with_storage(&context, dir.path().to_path_buf());
    let output = settle(&context, &mut fresh, size);
    let row = label(&output, "TEST.QWK");
    let forget = egui::pos2(row.left() - 40.0 + 460.0 - 18.0, row.bottom());
    frame(&context, &mut fresh, size, vec![egui::Event::PointerMoved(forget)]);
    for pressed in [true, false] {
        frame(&context, &mut fresh, size, pointer(forget, pressed));
    }
    assert!(fresh.recent.as_ref().unwrap().packets.is_empty());
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
    let search = timed("search keystroke", 3, |frame| {
        mail.reader.filter = format!("topic {frame}");
        mail.filter_changed();
    });
    assert!(search < budget * 10, "search took {search:?}");
    mail.reader.filter.clear();
    mail.filter_changed();
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
                egui::Event::Key { modifiers, .. } => Some(*modifiers),
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
fn gpu_mail_layout_themes_narrow_and_hidpi() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
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
        label(&output, "TEST.QWK");
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_long_message_scroll_and_selection_copy() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
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
