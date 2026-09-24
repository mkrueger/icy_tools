use super::*;
use egui_wgpu::wgpu;

struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
    context: egui::Context,
    time: f64,
    controls: std::collections::HashMap<String, egui::Pos2>,
    text_bounds: std::collections::HashMap<String, egui::Rect>,
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_welcome_screen_startup() {
    let mut harness = Harness::new().await;
    let screen = icy_term::welcome_screen::create_welcome_screen(None);
    let position = screen.caret.position();
    assert_eq!(position.x, 0);
    assert!(position.y > 1);
    let mut app = TerminalApp::new(screen, "Icy Term".into());
    for (size, scale, name) in [([1000, 720], 1.0, "desktop"), ([360, 640], 1.0, "narrow"), ([1600, 1200], 2.0, "hidpi")] {
        harness.capture(&mut app, size, scale, vec![], "welcome-warmup");
        let pixels = harness.capture(&mut app, size, scale, vec![], &format!("welcome-{name}"));
        assert!(pixels.chunks_exact(4).collect::<std::collections::HashSet<_>>().len() > 20);
        assert_eq!(app.terminal.screen.lock().caret().position(), position);
        assert!(app.session.is_none());
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_messages_layout_and_keyboard() {
    let mut harness = Harness::new().await;
    for (theme, theme_name) in [(egui::ThemePreference::Dark, "dark"), (egui::ThemePreference::Light, "light")] {
        harness.context.set_theme(theme);
        let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
        app.error = Some("Could not save the connection profile.\n".repeat(30));
        for (size, scale, name) in [
            ([1000, 800], 1.0, "desktop"),
            ([360, 640], 1.0, "narrow"),
            ([1600, 1200], 2.0, "hidpi"),
            ([360, 240], 1.0, "short"),
        ] {
            harness.capture(&mut app, size, scale, vec![], "message-warmup");
            harness.capture(&mut app, size, scale, vec![], &format!("message-{theme_name}-{name}"));
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale));
            for label in [tr!("egui-message-error"), "OK".into(), tr!("terminal-menu-copy")] {
                assert!(harness.controls.contains_key(&label), "missing {label} on {name}");
                assert!(screen.contains_rect(harness.text_bounds[&label]), "clipped {label} on {name}");
            }
            assert!(app.blocks_terminal());
        }
        let fixture = phonebook::tests::Fixture::new();
        app.dialing_directory.phonebook = Some(fixture.load());
        app.dialing_directory.open = true;
        harness.capture(&mut app, [1000, 800], 1.0, vec![], "message-directory");
        harness.capture(&mut app, [1000, 800], 1.0, vec![key_event(egui::Key::Escape, true)], "message-dismiss");
        assert!(!app.messages.is_open());
        assert!(app.dialing_directory.open, "Escape closed the directory behind the message");
        assert!(app.session.is_none(), "message input leaked into modem");
        app.dialing_directory.open = false;
        harness.capture(&mut app, [1000, 800], 1.0, vec![key_event(egui::Key::Escape, false)], "message-release");
        app.confirm_close = true;
        harness.capture(&mut app, [1000, 800], 1.0, vec![], "question-focus");
        harness.capture(&mut app, [1000, 800], 1.0, vec![], &format!("question-{theme_name}"));
        harness.capture(&mut app, [1000, 800], 1.0, vec![key_event(egui::Key::Enter, true)], "question-enter");
        assert!(!app.closed, "Enter must not confirm destructive action by default");
        assert!(!app.confirm_close, "default action must cancel");
        assert!(app.session.is_none(), "question input leaked into modem");
    }
}

fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_quick_connect_layout() {
    let mut harness = Harness::new().await;
    let fixture = phonebook::tests::Fixture::new();
    let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
    app.dialing_directory.phonebook = Some(fixture.load());
    app.dialing_directory.open = true;
    harness.capture(&mut app, [1000, 800], 1.0, vec![], "quick-list-warmup");
    harness.capture(&mut app, [1000, 800], 1.0, vec![], "quick-list");
    let position = harness.controls[&tr!("dialing_directory-connect-to-address")];
    for pressed in [true, false] {
        harness.capture(
            &mut app,
            [1000, 800],
            1.0,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            "quick-select",
        );
    }
    for (size, scale, name) in [
        ([1000, 800], 1.0, "desktop"),
        ([360, 640], 1.0, "narrow"),
        ([1600, 1200], 2.0, "hidpi"),
        ([360, 240], 1.0, "short"),
    ] {
        harness.capture(&mut app, size, scale, vec![], "quick-warmup");
        harness.capture(&mut app, size, scale, vec![], &format!("quick-{name}"));
        for label in [tr!("egui-quick-connect"), tr!("dialing_directory-add-bbs-button")] {
            assert!(harness.controls.contains_key(&label), "missing {label} on {name}");
        }
    }
    assert!(app.dialing_directory.phonebook.as_ref().unwrap().book.addresses.is_empty());
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_diagnostic_hover_details() {
    let mut harness = Harness::new().await;
    for (theme, theme_name) in [(egui::ThemePreference::Dark, "dark"), (egui::ThemePreference::Light, "light")] {
        harness.context.set_theme(theme);
        let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
        app.terminal_info();
        for (label, detail) in [
            (tr!("terminal-info-dialog-mouse-tracking"), tr!("terminal-info-dialog-mouse-mode-tooltip-off")),
            (tr!("terminal-info-dialog-caret-shape"), tr!("terminal-info-dialog-shape-tooltip-block")),
            (tr!("egui-info-kitty"), tr!("terminal-info-dialog-not-set")),
        ] {
            harness.capture(&mut app, [1000, 768], 1.0, vec![egui::Event::PointerGone], "hover-clear");
            harness.capture(&mut app, [1000, 768], 1.0, vec![], "hover-layout");
            let position = harness.controls[&label];
            harness.capture(&mut app, [1000, 768], 1.0, vec![egui::Event::PointerMoved(position)], "hover-enter");
            harness.time += 1.0;
            harness.capture(&mut app, [1000, 768], 1.0, vec![], "hover-warmup");
            harness.capture(&mut app, [1000, 768], 1.0, vec![], &format!("hover-{theme_name}-{label}"));
            assert!(harness.controls.contains_key(&detail), "missing hover detail: {detail}");
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 768.0)).contains_rect(harness.text_bounds[&detail]));
            assert!(app.tools.terminal_info.is_some() && app.session.is_none());
        }
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_main_menu_shows_shortcuts() {
    let size = [1000, 720];
    let mut harness = Harness::new().await;
    let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
    harness.capture(&mut app, size, 1.0, vec![], "menu-warmup");
    harness.capture(&mut app, size, 1.0, vec![], "menu-toolbar");
    let toolbar_y = harness.controls[&tr!("egui-directory")].y;
    let hamburger = egui::pos2(size[0] as f32 - 22.0, toolbar_y);
    for pressed in [true, false] {
        harness.capture(
            &mut app,
            size,
            1.0,
            vec![
                egui::Event::PointerMoved(hamburger),
                egui::Event::PointerButton {
                    pos: hamburger,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            "menu-click",
        );
    }
    harness.capture(&mut app, size, 1.0, vec![], "menu-root");
    assert!(
        !harness.controls.contains_key(&tr!("egui-quick-connect")),
        "quick connect must not be part of the main menu"
    );
    for entry in [tr!("egui-file"), tr!("egui-edit"), tr!("egui-view"), tr!("egui-session")] {
        assert!(harness.controls.contains_key(&entry), "missing menu {entry}");
    }
    for (submenu, shortcut) in [
        (tr!("egui-file"), hotkeys::shortcut(hotkeys::Action::Settings)),
        (tr!("egui-edit"), hotkeys::shortcut(hotkeys::Action::Find)),
        (tr!("egui-view"), hotkeys::shortcut(hotkeys::Action::Fullscreen)),
        (tr!("egui-session"), hotkeys::shortcut(hotkeys::Action::Upload)),
    ] {
        let position = harness.controls[&submenu];
        harness.capture(&mut app, size, 1.0, vec![egui::Event::PointerMoved(position)], "menu-hover");
        harness.time += 1.0;
        harness.capture(&mut app, size, 1.0, vec![], "menu-submenu-warmup");
        harness.capture(&mut app, size, 1.0, vec![], &format!("menu-{submenu}"));
        assert!(harness.controls.contains_key(&shortcut), "missing shortcut {shortcut} in {submenu}");
    }
    assert!(app.session.is_none());
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_find_overlay_preserves_terminal_area() {
    use icy_engine::EditableScreen;
    let mut harness = Harness::new().await;
    for (size, scale, name) in [([1000, 720], 1.0, "desktop"), ([360, 240], 1.0, "short"), ([1600, 1200], 2.0, "hidpi")] {
        let mut screen = TextScreen::default();
        for (column, character) in "FIND FIND FIND".chars().enumerate() {
            screen.set_char((column as i32, 0).into(), icy_engine::AttributedChar::new(character, Default::default()));
        }
        let mut app = TerminalApp::new(screen, "Icy Term".into());
        harness.capture(&mut app, size, scale, vec![], "find-baseline-warmup");
        harness.capture(&mut app, size, scale, vec![], "find-baseline");
        let before = app.terminal.render_info.read().clone();
        app.navigation.find_open = true;
        harness.capture(&mut app, size, scale, vec![], "find-open");
        harness.capture(&mut app, size, scale, vec![egui::Event::Text("FIND".into())], "find-typing");
        harness.capture(&mut app, size, scale, vec![], &format!("find-overlay-{name}"));
        let after = app.terminal.render_info.read().clone();
        assert_eq!(
            (before.bounds_width, before.bounds_height, before.display_scale),
            (after.bounds_width, after.bounds_height, after.display_scale)
        );
        let search = harness.text_bounds["FIND"];
        assert!(search.top() < after.bounds_y + 40.0, "search is not at top of terminal: {search:?}");
        if name == "desktop" {
            assert!(search.left() > 500.0, "search must float at right");
        }
        assert_eq!(app.navigation.search_result, Some((1, 3)));
        let counter = tr!("terminal-find-results", cur = "1".to_string(), total = "3".to_string());
        assert!(
            harness.controls.contains_key(&counter),
            "missing {counter:?}: {:?}",
            harness.controls.keys().collect::<Vec<_>>()
        );
        assert!(
            (harness.text_bounds["\u{00d7}"].center().y - search.center().y).abs() < 3.0,
            "find buttons wrapped"
        );
        app.error = Some("A message over the search".into());
        harness.capture(&mut app, size, scale, vec![], "find-message");
        harness.capture(&mut app, size, scale, vec![key_event(egui::Key::Escape, true)], "find-message-close");
        assert!(app.navigation.find_open, "Escape leaked behind message");
        assert!(!app.messages.is_open());
        assert!(app.session.is_none());
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_find_bar_keyboard_and_layout() {
    use icy_engine::EditableScreen;
    let mut harness = Harness::new().await;
    let mut screen = TextScreen::default();
    for (column, character) in "FIND FIND FIND".chars().enumerate() {
        screen.set_char((column as i32, 0).into(), icy_engine::AttributedChar::new(character, Default::default()));
    }
    let mut app = TerminalApp::new(screen, "Icy Term".into());
    app.navigation.find_open = true;
    app.navigation.query = "FIND".into();
    harness.capture(&mut app, [360, 240], 1.0, vec![], "find-warmup");
    harness.capture(&mut app, [360, 240], 1.0, vec![], "find-narrow");
    for label in ["FIND", "Aa", "\u{2191}", "\u{2193}", "\u{00d7}"] {
        assert!(harness.controls.contains_key(label), "missing find control {label}");
    }
    for (key, modifiers, expected) in [
        (egui::Key::Enter, egui::Modifiers::NONE, 0),
        (egui::Key::Enter, egui::Modifiers::NONE, 5),
        (egui::Key::Enter, egui::Modifiers::SHIFT, 0),
    ] {
        harness.capture(
            &mut app,
            [360, 240],
            1.0,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            "find-enter",
        );
        assert_eq!(app.terminal.screen.lock().selection().unwrap().anchor.x, expected);
        harness.capture(
            &mut app,
            [360, 240],
            1.0,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            }],
            "find-release",
        );
    }
    harness.capture(
        &mut app,
        [360, 240],
        1.0,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        "find-close",
    );
    assert!(!app.navigation.find_open);
    assert!(app.session.is_none(), "find keys leaked into the offline modem");
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_remaining_dialogs_fit_viewport() {
    let mut harness = Harness::new().await;
    for (theme, theme_name) in [(egui::ThemePreference::Dark, "dark"), (egui::ThemePreference::Light, "light")] {
        harness.context.set_theme(theme);
        for name in [
            "monitor",
            "live-terminal",
            "serial",
            "lua",
            "iemsi",
            "about",
            "help",
            "bps",
            "link",
            "close",
            "transfer",
            "save-screen",
            "capture",
        ] {
            let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
            let footer = match name {
                "monitor" => {
                    app.show_monitor = true;
                    tr!("egui-reset-monitor")
                }
                "live-terminal" => {
                    app.tools.terminal = Some(icy_term::Address::default());
                    tr!("egui-cancel")
                }
                "serial" => {
                    app.tools.serial_open = true;
                    tr!("dialing_directory-connect-button")
                }
                "lua" => {
                    app.tools.script_code = Some("print('Icy Term')".into());
                    app.tools.script_result = Some("Ready".into());
                    tr!("egui-close")
                }
                "iemsi" => {
                    app.tools.info_open = true;
                    app.tools.host_info = Some(vec![
                        (tr!("show-iemsi-dialog-name"), "Northern Lights BBS".into()),
                        (
                            tr!("show-iemsi-dialog-notice"),
                            "A long system notice that wraps across multiple lines without obscuring the actions below.".into(),
                        ),
                    ]);
                    tr!("egui-close")
                }
                "about" => {
                    app.about_open = true;
                    tr!("egui-close")
                }
                "help" => {
                    app.help_open = true;
                    tr!("egui-close")
                }
                "bps" => {
                    app.bps_open = true;
                    tr!("egui-close")
                }
                "link" => {
                    app.pending_link = Some("https://example.com/a-long-path-to-a-bbs-directory-entry-and-other-information".into());
                    tr!("egui-cancel")
                }
                "close" => {
                    app.confirm_close = true;
                    tr!("egui-cancel")
                }
                "transfer" => {
                    app.transfers.choose(true);
                    tr!("egui-close")
                }
                "save-screen" => {
                    app.save_screen = Some(export::SaveScreen::new(""));
                    tr!("egui-save")
                }
                "capture" => {
                    app.capture = Some(export::Capture::new("", false));
                    tr!("egui-capture-start")
                }
                _ => unreachable!(),
            };
            for (size, scale, viewport) in [
                ([1000, 768], 1.0, "desktop"),
                ([360, 640], 1.0, "narrow"),
                ([1600, 1200], 2.0, "hidpi"),
                ([360, 240], 1.0, "short"),
            ] {
                harness.capture(&mut app, size, scale, vec![], "dialog-warmup");
                if name == "serial" && viewport == "desktop" {
                    harness.capture(&mut app, size, scale, vec![], "dialog-warmup");
                }
                harness.capture(&mut app, size, scale, vec![], &format!("{theme_name}-{viewport}-{name}"));
                assert!(harness.controls.contains_key(&footer), "missing {footer}: {theme_name}-{viewport}-{name}");
                let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale));
                assert!(screen.contains_rect(harness.text_bounds[&footer]), "footer outside viewport: {name}");
                if matches!(name, "serial" | "live-terminal" | "save-screen" | "capture") && viewport == "desktop" {
                    let former_title = match name {
                        "serial" => tr!("egui-serial-connection"),
                        "live-terminal" => tr!("egui-terminal-settings"),
                        "save-screen" => tr!("egui-save-screen").trim_end_matches("...").to_owned(),
                        _ => tr!("egui-captures"),
                    };
                    assert!(!harness.controls.contains_key(&former_title), "redundant dialog title: {name}");
                    let first_field = match name {
                        "serial" => tr!("settings-modem-device"),
                        "live-terminal" => tr!("egui-terminal-emulation"),
                        "save-screen" => tr!("egui-format"),
                        _ => tr!("settings-paths-download-dir"),
                    };
                    let field = harness.text_bounds.get(&first_field).unwrap_or_else(|| panic!("missing {first_field}: {name}"));
                    assert!(field.bottom() < harness.text_bounds[&footer].top(), "form overlaps footer: {name}");
                    if name == "serial" {
                        let last_field = tr!("settings-modem-flow_control");
                        assert!(
                            harness
                                .text_bounds
                                .get(&last_field)
                                .is_some_and(|field| field.bottom() < harness.text_bounds[&footer].top()),
                            "serial fields are clipped: {theme_name}"
                        );
                    }
                }
                if name == "bps" && viewport == "desktop" {
                    assert!(
                        harness.controls.contains_key(&tr!("select-bps-dialog-bps-custom")),
                        "custom BPS field is clipped"
                    );
                }
            }
        }
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_terminal_information_applies_mode() {
    let mut harness = Harness::new().await;
    let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
    app.terminal_info();
    harness.capture(&mut app, [1014, 768], 1.0, vec![], "apply-warmup");
    harness.capture(&mut app, [1014, 768], 1.0, vec![], "apply-info");
    for label in ["VGA 80x25".to_string(), "VGA 80x50".into(), tr!("terminal-info-dialog-apply-button")] {
        let position = *harness.controls.get(&label).unwrap_or_else(|| panic!("missing {label}"));
        for pressed in [true, false] {
            harness.capture(
                &mut app,
                [1014, 768],
                1.0,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                "apply-click",
            );
        }
        harness.capture(&mut app, [1014, 768], 1.0, vec![], "apply-settle");
    }
    assert!(app.tools.terminal_info.is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.terminal.screen.lock().height() != 50 {
        assert!(std::time::Instant::now() < deadline, "mode change was not applied: {:?}", app.error);
        app.receive_events(&harness.context);
        std::thread::yield_now();
    }
    assert_eq!(app.terminal.screen.lock().width(), 80);
    app.receive_events(&harness.context);
    assert_eq!(app.terminal_profile().screen_mode, icy_engine::ScreenMode::Vga(80, 50));
    app.screen_mode = icy_engine::ScreenMode::AtariST(icy_engine::TerminalResolution::High, true);
    app.terminal_emulation = icy_net::telnet::TerminalEmulation::AtariST;
    app.ansi_music = icy_parser_core::MusicOption::Both;
    let profile = app.terminal_profile();
    assert_eq!(profile.screen_mode, app.screen_mode);
    assert_eq!(profile.ansi_music, app.ansi_music);
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_terminal_information_layout() {
    let mut harness = Harness::new().await;
    let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
    for (theme, theme_name) in [(egui::ThemePreference::Dark, "dark"), (egui::ThemePreference::Light, "light")] {
        harness.context.set_theme(theme);
        app.terminal_info();
        for (size, scale, name) in [
            ([1014, 768], 1.0, "desktop"),
            ([360, 640], 1.0, "narrow"),
            ([1600, 1200], 2.0, "hidpi"),
            ([360, 240], 1.0, "short"),
        ] {
            harness.capture(&mut app, size, scale, vec![], "info-warmup");
            harness.capture(&mut app, size, scale, vec![], &format!("{theme_name}-{name}-terminal-info"));
            for label in [tr!("egui-close"), tr!("terminal-info-dialog-apply-button"), tr!("terminal-menu-copy")] {
                assert!(harness.controls.contains_key(&label), "missing {label}: {name}");
            }
            if name == "desktop" {
                for label in [
                    tr!("terminal-info-dialog-caret-section"),
                    tr!("terminal-info-dialog-auto-wrap"),
                    tr!("egui-info-input-protocols"),
                    tr!("egui-info-graphics-protocols"),
                    tr!("egui-info-sync-output"),
                    tr!("egui-terminal-emulation"),
                    "VGA 80x25".into(),
                ] {
                    assert!(harness.controls.contains_key(&label), "missing {label}");
                }
                let size = harness.text_bounds[&tr!("terminal-info-dialog-resolution")];
                let value = harness.text_bounds["80x25"];
                let note = harness.text_bounds["(640x400 px)"];
                let cursor = harness.text_bounds[&tr!("terminal-info-dialog-caret-position")];
                assert!(note.left() >= value.right() && note.height() < value.height());
                assert!((size.top() - value.top()).abs() < 2.0 && size.right() < value.left());
                assert!(cursor.left() > value.right() && (cursor.top() - size.top()).abs() < 2.0);
                assert!(!harness.controls.contains_key("false") && !harness.controls.contains_key("true"));
            }
        }
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_overlays_and_shortcut_actions() {
    use icy_engine::{EditableScreen, Screen};
    let mut harness = Harness::new().await;
    let mut screen = TextScreen::default();
    for (column, character) in "HISTORY".chars().enumerate() {
        screen.set_char((column as i32, 0).into(), icy_engine::AttributedChar::new(character, Default::default()));
    }
    screen.terminal_state_mut().is_terminal_buffer = true;
    screen.set_scrollback_buffer_size(400);
    for _ in 0..40 {
        screen.scroll_up();
    }
    let mut app = TerminalApp::new(screen, "Icy Term".into());
    app.focus_terminal = true;
    app.tools.host_info = Some(vec![("Name".into(), "Test BBS".into())]);

    // The status bar exposes the legacy overlay controls.
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-warmup");
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-status");
    assert!(harness.controls.contains_key("LOCAL"), "missing BPS control");
    assert!(harness.controls.contains_key("IEMSI"), "missing IEMSI control");
    assert!(
        harness.controls.keys().any(|label| label.contains('\u{2022}')),
        "missing clickable terminal info"
    );

    // The scrollback overlay reports the distance from the live screen.
    app.shortcut(hotkeys::Action::Scrollback, &harness.context.clone());
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-history-warmup");
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-history");
    assert!(app.terminal.is_in_scrollback_mode());
    let indicator = harness
        .controls
        .keys()
        .find(|label| label.starts_with('\u{2191}'))
        .expect("missing scrollback indicator")
        .clone();
    let info = app.terminal.render_info.read().clone();
    let bounds = harness.text_bounds[&indicator];
    let widget = egui::Rect::from_min_size(egui::pos2(info.bounds_x, info.bounds_y), egui::vec2(info.bounds_width, info.bounds_height));
    assert!(
        widget.contains_rect(bounds) && bounds.min.x > widget.center().x && bounds.min.y < widget.min.y + 60.0,
        "indicator {bounds:?} is not in the top right of {widget:?}"
    );
    app.shortcut(hotkeys::Action::Scrollback, &harness.context.clone());

    // The right-click menu offers the same entries as the legacy client.
    let center = egui::pos2(info.bounds_x + info.viewport_x + 40.0, info.bounds_y + info.viewport_y + 40.0);
    for pressed in [true, false] {
        harness.capture(
            &mut app,
            [1000, 720],
            1.0,
            vec![
                egui::Event::PointerMoved(center),
                egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            "overlay-context-click",
        );
    }
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-context-menu");
    for entry in [
        tr!("terminal-dialing_directory"),
        tr!("terminal-upload"),
        tr!("terminal-download"),
        tr!("terminal-menu-copy"),
        tr!("terminal-menu-paste"),
        tr!("terminal-menu-info"),
    ] {
        assert!(harness.controls.contains_key(&entry), "missing context entry {entry}");
    }
    harness.capture(
        &mut app,
        [1000, 720],
        1.0,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        "overlay-context-close",
    );

    // Shortcuts open the shared dialogs and list the same keys as the command table.
    for (action, expected) in [(hotkeys::Action::Help, tr!("help-title")), (hotkeys::Action::About, tr!("egui-close"))] {
        app.shortcut(action, &harness.context.clone());
        harness.capture(&mut app, [360, 640], 1.0, vec![], "overlay-dialog-warmup");
        harness.capture(&mut app, [360, 640], 1.0, vec![], &format!("overlay-{action:?}"));
        assert!(harness.controls.contains_key(&expected), "{action:?} did not open");
        assert!(app.blocks_terminal(), "{action:?} must hold the keyboard");
        app.help_open = false;
        app.about_open = false;
    }
    app.shortcut(hotkeys::Action::Help, &harness.context.clone());
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-help-warmup");
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-help");
    for shortcut in [
        hotkeys::shortcut(hotkeys::Action::DialingDirectory),
        hotkeys::shortcut(hotkeys::Action::Hangup),
        hotkeys::shortcut(hotkeys::Action::Upload),
    ] {
        // Each key is drawn as its own pill, so look for the individual keys.
        for key in hotkeys::key_parts(&shortcut) {
            assert!(harness.controls.contains_key(&key), "help is missing {key} of {shortcut}");
        }
    }
    for (_, entries) in hotkeys::help_entries() {
        for entry in entries {
            assert!(!entry.action.contains("No localization"), "untranslated shortcut label: {}", entry.action);
            assert!(
                !entry.description.contains("No localization"),
                "untranslated shortcut description: {}",
                entry.description
            );
        }
    }
    app.help_open = false;

    // Zoom and clear are wired to the terminal, not just to the menu.
    app.shortcut(hotkeys::Action::ZoomReset, &harness.context.clone());
    assert!(matches!(app.settings.scaling_mode, ScalingMode::Manual(zoom) if (zoom - 1.0).abs() < f32::EPSILON));
    app.shortcut(hotkeys::Action::ZoomFit, &harness.context.clone());
    assert!(app.settings.scaling_mode.is_auto());
    app.shortcut(hotkeys::Action::ClearScreen, &harness.context.clone());
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "overlay-cleared");
    assert_eq!(app.terminal.screen.lock().char_at((0, 0).into()).ch, ' ');
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_ui_themes_and_text_layout() {
    let mut harness = Harness::new().await;
    for (theme, name) in [(egui::ThemePreference::Dark, "dark"), (egui::ThemePreference::Light, "light")] {
        harness.context.set_theme(theme);
        let screen = FileFormat::IcyDraw
            .from_bytes(include_bytes!("../../../data/welcome_screen.1.icy"), None)
            .unwrap()
            .screen;
        let mut app = TerminalApp::new(screen, "Icy Term".into());
        for (size, scale, viewport) in [([1000, 720], 1.0, "desktop"), ([360, 640], 1.0, "narrow"), ([1600, 1200], 2.0, "hidpi")] {
            app.preferences = None;
            harness.capture(&mut app, size, scale, vec![], "theme-warmup");
            harness.capture(&mut app, size, scale, vec![], &format!("{name}-{viewport}-welcome"));
            assert!(app.terminal.render_info.read().bounds_y <= 44.0);
            let path = std::env::temp_dir().join(format!("icy-theme-{}.toml", fastrand::u64(..)));
            app.preferences = Some(settings::Settings::open(&icy_term::Options::default(), path).unwrap());
            // The dialog opens on Monitor like the legacy client; this check targets the Terminal form.
            app.preferences.as_mut().unwrap().page = settings::Page::Terminal;
            harness.capture(&mut app, size, scale, vec![], "form-warmup");
            harness.capture(&mut app, size, scale, vec![], &format!("{name}-{viewport}-form"));
            let mut labels = vec![
                tr!("egui-connect-timeout"),
                tr!("egui-scrollback-lines"),
                tr!("egui-cursor"),
                tr!("egui-appearance"),
                "1000".into(),
                "2000".into(),
                "Block".into(),
                tr!("egui-system"),
                tr!("settings-terminal-invert-mouse-wheel"),
                icy_engine_gui::LANGUAGE_LOADER.get("dialog-ok-button"),
                tr!("egui-cancel"),
            ];
            // Narrow windows stack the form rows, so the cursor options scroll below the fold.
            if viewport != "narrow" {
                labels.push(tr!("settings-terminal-cursor-blinking"));
            }
            for (index, label) in labels.iter().enumerate() {
                let bounds = harness.text_bounds.get(label).unwrap_or_else(|| panic!("Missing {label} in {name}-{viewport}"));
                for other in &labels[index + 1..] {
                    let other_bounds = harness.text_bounds.get(other).unwrap_or_else(|| panic!("Missing {other} in {name}-{viewport}"));
                    assert!(!bounds.shrink(1.0).intersects(*other_bounds), "{label} overlaps {other}: {name}-{viewport}");
                }
            }
        }
        app.preferences = None;
        let fixture = phonebook::tests::Fixture::new();
        let mut book = fixture.load();
        phonebook::tests::add(&mut book, "Northern Lights BBS");
        book.toggle_favorite(0).unwrap();
        app.dialing_directory.phonebook = Some(book);
        app.dialing_directory.open = true;
        harness.capture(&mut app, [1000, 720], 1.0, vec![], "theme-directory-warmup");
        harness.capture(&mut app, [1000, 720], 1.0, vec![], &format!("{name}-directory"));
        let facts = [
            tr!("dialing_directory-user"),
            tr!("dialing_directory-screen_mode"),
            tr!("egui-font"),
            tr!("egui-baud"),
            tr!("egui-calls"),
        ];
        for adjacent in facts.windows(2) {
            assert!(harness.text_bounds[&adjacent[0]].bottom() < harness.text_bounds[&adjacent[1]].top());
        }
        assert!((harness.text_bounds["VGA 80x25"].left() - harness.text_bounds["IBM VGA"].left()).abs() < 1.0);
        app.dialing_directory.open = false;
        harness.capture(&mut app, [360, 640], 1.0, vec![], "menu-warmup");
        let position = egui::pos2(330.0, 20.0);
        for pressed in [true, false] {
            harness.capture(
                &mut app,
                [360, 640],
                1.0,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                "menu-click",
            );
        }
        harness.capture(&mut app, [360, 640], 1.0, vec![], &format!("{name}-menu"));
        for label in [tr!("egui-file"), tr!("egui-view"), tr!("egui-edit"), tr!("egui-session")] {
            assert!(harness.controls.contains_key(&label), "Missing menu {label}");
        }
        harness.capture(
            &mut app,
            [360, 640],
            1.0,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            "menu-close",
        );
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_navigation_and_session_dialogs() {
    use icy_engine::EditableScreen;
    let mut harness = Harness::new().await;
    let mut screen = TextScreen::default();
    for (column, character) in "SELECT".chars().enumerate() {
        screen.set_char((column as i32, 0).into(), icy_engine::AttributedChar::new(character, Default::default()));
    }
    let mut app = TerminalApp::new(screen, "Selection".into());
    app.focus_terminal = true;
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "selection-warmup");
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "selection-layout");
    let info = app.terminal.render_info.read().clone();
    let origin = egui::pos2(
        info.bounds_x + info.viewport_x + info.font_width * info.display_scale * 0.5,
        info.bounds_y + info.viewport_y + info.font_height * info.display_scale * 0.5,
    );
    let end = origin + egui::vec2(info.font_width * info.display_scale * 5.0, 0.0);
    harness.capture(
        &mut app,
        [1000, 720],
        1.0,
        vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        "selection-press",
    );
    harness.capture(&mut app, [1000, 720], 1.0, vec![egui::Event::PointerMoved(end)], "selection-drag");
    harness.capture(
        &mut app,
        [1000, 720],
        1.0,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        "selection-release",
    );
    assert_eq!(navigation::selected_text(&**app.terminal.screen.lock()).as_deref(), Some("SELECT"));
    let path = std::env::temp_dir().join(format!("icy-settings-gpu-{}.toml", fastrand::u64(..)));
    app.preferences = Some(settings::Settings::open(&icy_term::Options::default(), path).unwrap());
    app.preferences.as_mut().unwrap().draft.modems.push(Default::default());
    for (size, scale, prefix) in [
        ([1000, 800], 1.0, "desktop"),
        ([360, 640], 1.0, "narrow"),
        ([1600, 1200], 2.0, "hidpi"),
        ([360, 240], 1.0, "short"),
    ] {
        for page in [
            settings::Page::Monitor,
            settings::Page::Terminal,
            settings::Page::Audio,
            settings::Page::Paths,
            settings::Page::Login,
            settings::Page::Serial,
            settings::Page::Sources,
            settings::Page::Modems,
            settings::Page::Protocols,
        ] {
            app.preferences.as_mut().unwrap().page = page;
            harness.capture(&mut app, size, scale, vec![], "settings-warmup");
            harness.capture(&mut app, size, scale, vec![], &format!("{prefix}-settings-{page:?}"));
            assert!(
                harness.controls.contains_key(&icy_engine_gui::LANGUAGE_LOADER.get("dialog-ok-button")) && harness.controls.contains_key(&*tr!("egui-cancel")),
                "Missing settings actions {prefix} {page:?}"
            );
        }
    }
    app.preferences = None;
    app.transfers.choose(true);
    for (size, prefix) in [([1000, 800], "desktop"), ([360, 640], "narrow"), ([360, 240], "short")] {
        harness.capture(&mut app, size, 1.0, vec![], "transfer-warmup");
        harness.capture(&mut app, size, 1.0, vec![], &format!("{prefix}-transfer"));
        assert!(harness.controls.contains_key(&*tr!("egui-close")));
    }
    app.transfers.open = false;
    app.tools.serial_open = true;
    harness.capture(&mut app, [360, 640], 1.0, vec![], "serial-warmup");
    harness.capture(&mut app, [360, 640], 1.0, vec![], "narrow-serial");
    assert!(harness.controls.contains_key(&*tr!("dialing_directory-connect-button")) && harness.controls.contains_key(&*tr!("egui-cancel")));
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_dialing_directory_layout() {
    let mut harness = Harness::new().await;
    let fixture = phonebook::tests::Fixture::new();
    let mut book = fixture.load();
    for name in [
        "Northern Lights BBS",
        "Retro Computing Network",
        "A very long system name that must stay inside the directory",
    ] {
        phonebook::tests::add(&mut book, name);
    }
    book.toggle_favorite(0).unwrap();
    let mut app = TerminalApp::new(TextScreen::default(), "Icy Term".into());
    app.dialing_directory.phonebook = Some(book);
    app.dialing_directory.open = true;
    for (size, scale, name) in [
        ([1000, 800], 1.0, "dial-desktop"),
        ([360, 640], 1.0, "dial-narrow"),
        ([1600, 1200], 2.0, "dial-hidpi"),
    ] {
        harness.capture(&mut app, size, scale, vec![], "dial-warmup");
        let pixels = harness.capture(&mut app, size, scale, vec![], name);
        let bounds = app.dialing_directory.bounds.unwrap();
        assert!(bounds.width() >= size[0] as f32 / scale - 34.0, "directory does not fill window: {bounds:?}");
        assert!(bounds.min.x >= -1.0 && bounds.min.y >= -1.0, "{bounds:?}");
        assert!(
            bounds.max.x <= size[0] as f32 / scale + 1.0 && bounds.max.y <= size[1] as f32 / scale + 1.0,
            "{bounds:?}"
        );
        assert!(pixels.chunks_exact(4).collect::<std::collections::HashSet<_>>().len() > 20);
    }
    app.dialing_directory.phonebook.as_mut().unwrap().begin_edit();
    harness.capture(&mut app, [1000, 800], 1.0, vec![], "dial-edit-warmup");
    harness.capture(&mut app, [1000, 800], 1.0, vec![], "dial-edit");
    harness.capture(&mut app, [360, 640], 1.0, vec![], "dial-edit-narrow-warmup");
    harness.capture(&mut app, [360, 640], 1.0, vec![], "dial-edit-narrow");
    let bounds = app.dialing_directory.bounds.unwrap();
    assert!(
        bounds.min.x >= -1.0 && bounds.min.y >= -1.0 && bounds.max.x <= 361.0 && bounds.max.y <= 641.0,
        "{bounds:?}"
    );
    for page in [
        &*tr!("egui-connection"),
        &*tr!("settings-terminal-category"),
        &*tr!("egui-login"),
        &*tr!("egui-colors"),
        &*tr!("dialing_directory-notes"),
    ] {
        let draft = app.dialing_directory.phonebook.as_mut().unwrap().draft.as_mut().unwrap();
        draft.custom_palette = Some(
            icy_engine::DOS_DEFAULT_PALETTE
                .iter()
                .map(|color| {
                    let (red, green, blue) = color.rgb();
                    [red, green, blue]
                })
                .collect(),
        );
        draft.proxy = Some(icy_net::proxy::ProxyConfig::socks5("127.0.0.1", 9050));
        draft.protocol = icy_net::ConnectionType::SSH;
        draft.ssh_authentication = icy_term::SshAuthenticationMode::PrivateKey;
        for (size, name) in [([1000, 800], "desktop"), ([360, 640], "narrow")] {
            harness.capture(&mut app, size, 1.0, vec![], "page-warmup");
            harness.capture(&mut app, size, 1.0, vec![], "page-layout");
            let position = *harness.controls.get(page).unwrap_or_else(|| panic!("Missing page {page}"));
            harness.capture(
                &mut app,
                size,
                1.0,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                "page-press",
            );
            harness.capture(
                &mut app,
                size,
                1.0,
                vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                "page-release",
            );
            harness.capture(&mut app, size, 1.0, vec![], &format!("{name}-{page}"));
            assert!(
                harness.controls.contains_key(&*tr!("egui-save")) && harness.controls.contains_key(&*tr!("egui-discard")),
                "Hidden actions on {name} {page}"
            );
            let bounds = app.dialing_directory.bounds.unwrap();
            assert!(
                bounds.min.x >= -1.0 && bounds.min.y >= -1.0 && bounds.max.x <= size[0] as f32 + 1.0 && bounds.max.y <= size[1] as f32 + 1.0,
                "{page} {bounds:?}"
            );
        }
    }
    harness.capture(&mut app, [360, 240], 1.0, vec![], "short-warmup");
    harness.capture(&mut app, [360, 240], 1.0, vec![], "short");
    let bounds = app.dialing_directory.bounds.unwrap();
    assert!(
        bounds.min.x >= -1.0 && bounds.min.y >= -1.0 && bounds.max.x <= 361.0 && bounds.max.y <= 241.0,
        "{bounds:?}"
    );
}

impl Harness {
    async fn new() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("GPU adapter required");
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await.unwrap();
        let mut renderer = egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, Default::default());
        renderer
            .callback_resources
            .insert(TerminalShaderRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm));
        eprintln!("egui GPU test: {:?}", adapter.get_info());
        Self {
            device,
            queue,
            renderer,
            context: egui::Context::default(),
            time: 0.0,
            controls: Default::default(),
            text_bounds: Default::default(),
        }
    }

    fn capture(&mut self, app: &mut TerminalApp, size: [u32; 2], scale: f32, events: Vec<egui::Event>, name: &str) -> Vec<u8> {
        let started = std::time::Instant::now();
        self.time += 0.1;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
            )),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(scale);
        input.modifiers = input
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. } | egui::Event::PointerButton { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let output = self.context.run(input, |context| app.show(context));
        let ui_finished = std::time::Instant::now();
        assert_eq!(output.pixels_per_point, scale);
        self.controls.clear();
        self.text_bounds.clear();
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                if shape.clip_rect.contains_rect(rect) {
                    self.controls.insert(text.galley.text().to_string(), rect.center());
                    self.text_bounds.insert(text.galley.text().to_string(), rect);
                }
            }
        }
        let jobs = self.context.tessellate(output.shapes, output.pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            self.renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: output.pixels_per_point,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("egui terminal test"),
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
        let view = texture.create_view(&Default::default());
        let stride = (size[0] * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui terminal readback"),
            size: u64::from(stride * size[1]),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let commands = self.renderer.update_buffers(&self.device, &self.queue, &mut encoder, &jobs, &screen);
        let prepared = std::time::Instant::now();
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui terminal test pass"),
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
            self.renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
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
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receiver.recv().unwrap().unwrap();
        if std::env::var_os("ICY_EGUI_TIMINGS").is_some() {
            eprintln!(
                "{name} {size:?}: ui={:?} prepare={:?} gpu/readback={:?}",
                ui_finished - started,
                prepared - ui_finished,
                prepared.elapsed()
            );
        }
        let pixels: Vec<u8> = buffer
            .slice(..)
            .get_mapped_range()
            .chunks(stride as usize)
            .flat_map(|row| row[..size[0] as usize * 4].iter().copied())
            .collect();
        buffer.unmap();
        for id in output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
        if let Some(directory) = std::env::var_os("ICY_EGUI_SCREENSHOTS") {
            let directory = PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            image::save_buffer(directory.join(format!("{name}.png")), &pixels, size[0], size[1], image::ColorType::Rgba8).unwrap();
        }
        pixels
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_auto_resize_reuses_tiles_and_settles_after_one_frame() {
    use egui_wgpu::CallbackTrait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut harness = Harness::new().await;
    let screen = FileFormat::IcyDraw
        .from_bytes(include_bytes!("../../../data/welcome_screen.1.icy"), None)
        .unwrap()
        .screen;
    let mut app = TerminalApp::new(screen, "resize".into());
    let mut previous_tiles: Vec<Arc<Vec<u8>>> = Vec::new();
    let mut previous_generation = None;
    let mut previous_scale = 0.0;
    for size in [[1000, 720], [1004, 724], [1100, 800], [640, 480], [1920, 1200], [360, 240]] {
        harness.capture(&mut app, size, 1.0, vec![], "resize-cache");
        let info = app.terminal.render_info.read().clone();
        let bounds = egui::Rect::from_min_size(egui::pos2(info.bounds_x, info.bounds_y), egui::vec2(info.bounds_width, info.bounds_height));
        let frame = CRTShaderProgram::new(&app.terminal, Arc::new(app.settings.clone()), None).frame(&app.shader_state, [bounds.width(), bounds.height()], 1.0);
        if !previous_tiles.is_empty() {
            assert_eq!(previous_tiles.len(), frame.slices_blink_off.len());
            for (previous, current) in previous_tiles.iter().zip(&frame.slices_blink_off) {
                assert!(Arc::ptr_eq(previous, &current.rgba_data), "resize rerasterized terminal content");
            }
            assert_eq!(previous_generation, Some(frame.render_generation));
        }
        previous_tiles = frame.slices_blink_off.iter().map(|slice| slice.rgba_data.clone()).collect();
        previous_generation = Some(frame.render_generation);
        // A pristine context makes the very first repaint request observable.
        let probe = egui::Context::default();
        let repaints = Arc::new(AtomicUsize::new(0));
        let counter = repaints.clone();
        probe.set_request_repaint_callback(move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        let callback = TerminalCallback { frame, bounds, context: probe };
        app.terminal.render_info.write().display_scale = previous_scale;
        let mut encoder = harness.device.create_command_encoder(&Default::default());
        let mut prepare = |callback: &TerminalCallback, encoder: &mut wgpu::CommandEncoder| {
            callback.prepare(
                &harness.device,
                &harness.queue,
                &egui_wgpu::ScreenDescriptor {
                    size_in_pixels: size,
                    pixels_per_point: 1.0,
                },
                encoder,
                &mut harness.renderer.callback_resources,
            );
        };
        prepare(&callback, &mut encoder);
        let settled_scale = app.terminal.render_info.read().display_scale;
        if settled_scale != previous_scale {
            assert!(repaints.load(Ordering::Relaxed) > 0, "a changed scale must schedule the frame that shows it");
        }
        repaints.store(0, Ordering::Relaxed);
        prepare(&callback, &mut encoder);
        assert_eq!(repaints.load(Ordering::Relaxed), 0, "a settled scale must not keep scheduling frames");
        previous_scale = settled_scale;
    }
}

#[tokio::test]
#[ignore = "requires a working wgpu adapter"]
async fn gpu_render_resize_scroll_and_effects() {
    let mut harness = Harness::new().await;
    let welcome = FileFormat::IcyDraw
        .from_bytes(include_bytes!("../../../data/welcome_screen.1.icy"), None)
        .unwrap()
        .screen;
    let mut app = TerminalApp::new(welcome, "Icy Term".into());
    for (size, scale, name) in [([1000, 720], 1.0, "desktop"), ([360, 640], 1.0, "narrow"), ([1600, 1200], 2.0, "hidpi")] {
        harness.capture(&mut app, size, scale, vec![], "warmup");
        let pixels = harness.capture(&mut app, size, scale, vec![], name);
        let center: Vec<_> = pixels
            .chunks_exact((size[0] * 4) as usize)
            .skip((size[1] / 4) as usize)
            .take((size[1] / 2) as usize)
            .flatten()
            .copied()
            .collect();
        let colors: std::collections::HashSet<_> = center.chunks_exact(4).collect();
        let colored_pixels = center
            .chunks_exact(4)
            .filter(|pixel| pixel[..3].iter().max().unwrap() - pixel[..3].iter().min().unwrap() > 80)
            .count();
        assert!(colors.len() > 4 && colored_pixels > 100, "blank or missing terminal at {size:?}");
        let info = app.terminal.render_info.read();
        assert!(
            (info.viewport_width / info.viewport_height - info.terminal_width / info.terminal_height).abs() < 0.01,
            "Fit must preserve aspect ratio"
        );
    }
    app.settings.use_integer_scaling = false;
    app.settings.use_bilinear_filtering = false;
    app.settings.scaling_mode = ScalingMode::Manual(1.0);
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "neutral-warmup");
    let neutral = harness.capture(&mut app, [1000, 720], 1.0, vec![], "neutral");
    app.settings.use_scanlines = true;
    app.settings.use_curvature = true;
    let crt = harness.capture(&mut app, [1000, 720], 1.0, vec![], "crt");
    assert!(neutral != crt, "CRT effects must change the rendered pixels");
    assert!(neutral[..1000 * 20 * 4] == crt[..1000 * 20 * 4], "terminal must not paint over toolbar");
    app.show_monitor = true;
    harness.capture(&mut app, [1000, 720], 1.0, vec![], "dialog-warmup");
    let dialog = harness.capture(&mut app, [1000, 720], 1.0, vec![], "dialog");
    assert!(crt != dialog, "monitor window must paint above terminal");

    let ansi: String = (0..300)
        .map(|row| format!("\x1b[{}m{row:03} SCROLL TEST {}\x1b[0m\r\n", 41 + row % 6, "0123456789".repeat(5)))
        .collect();
    let screen = FileFormat::Ansi.from_bytes(ansi.as_bytes(), None).unwrap().screen;
    let mut app = TerminalApp::new(screen, "Scroll test".into());
    app.settings.scaling_mode = ScalingMode::Manual(2.0);
    harness.capture(&mut app, [800, 600], 1.0, vec![], "scroll-warmup");
    let top = harness.capture(&mut app, [800, 600], 1.0, vec![], "scroll-top");
    let events = vec![
        egui::Event::PointerMoved(egui::pos2(400.0, 300.0)),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(-200.0, -2400.0),
            modifiers: egui::Modifiers::NONE,
        },
    ];
    harness.capture(&mut app, [800, 600], 1.0, events, "scroll-event");
    let scrolled = harness.capture(&mut app, [800, 600], 1.0, vec![], "scrolled");
    assert!(app.terminal.scroll_y() > 0.0, "wheel must scroll vertically");
    assert!(app.terminal.scroll_x() > 0.0, "wheel must scroll horizontally");
    assert!(top != scrolled, "scrolling must change terminal pixels");
    let info = app.terminal.render_info.read();
    assert_eq!(info.display_scale, 2.0);
    assert!(info.viewport_width <= 800.0 && info.viewport_height <= 600.0);
}
