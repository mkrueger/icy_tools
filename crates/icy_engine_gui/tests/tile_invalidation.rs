//! Edits must repaint the cached tiles that actually contain the changed lines.
//!
//! Tiles are rendered in raw texture pixels (font bitmap height), while
//! `Screen::font_dimensions()` is aspect-ratio corrected. Mapping dirty lines
//! with the corrected height, or assuming a tile holds a whole number of lines,
//! leaves stale tiles on screen.

use std::sync::Arc;

use icy_engine::{AttributedChar, EditableScreen, Position, Screen, TextAttribute, TextScreen};
use icy_engine_gui::{CRTShaderProgram, CRTShaderState, MonitorSettings, ScalingMode, Terminal, TerminalShader, TILE_HEIGHT};
use parking_lot::Mutex;

fn terminal(height: i32, configure: impl FnOnce(&mut TextScreen)) -> (Terminal, CRTShaderState) {
    let mut screen = TextScreen::new((80, height));
    configure(&mut screen);
    let screen: Box<dyn Screen> = Box::new(screen);
    let state = CRTShaderState::from_screen(&*screen);
    (Terminal::new(Arc::new(Mutex::new(screen))), state)
}

fn frame(terminal: &Terminal, state: &CRTShaderState) -> TerminalShader {
    let settings = MonitorSettings {
        scaling_mode: ScalingMode::Manual(1.0),
        use_integer_scaling: false,
        ..Default::default()
    };
    CRTShaderProgram::new(terminal, Arc::new(settings), None).frame(state, [1000.0, 800.0], 1.0)
}

fn set_line(terminal: &Terminal, line: i32) {
    let mut screen = terminal.screen.lock();
    let editable = screen.as_editable().unwrap();
    for x in 0..80 {
        editable.set_char(Position::new(x, line), AttributedChar::new('\u{DB}', TextAttribute::default()));
    }
}

/// Row `row` (raw pixels) of tile `layer` contains any non-black pixel.
fn row_has_content(frame: &TerminalShader, layer: usize, row: u32) -> bool {
    let slice = &frame.slices_blink_off[layer];
    let stride = slice.width as usize * 4;
    let start = row as usize * stride;
    slice.rgba_data[start..start + stride]
        .chunks_exact(4)
        .any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0)
}

#[test]
fn aspect_ratio_edit_repaints_the_tile_containing_the_line() {
    let (terminal, state) = terminal(300, |screen| screen.set_aspect_ratio(true));
    assert_ne!(terminal.screen.lock().font_dimensions().height, 16, "aspect ratio correction must be active");
    let before = frame(&terminal, &state);
    // Raw rows 1840..1856 are in tile 0, but 115 * 19 = 2185 would place the line in tile 1.
    let line = 115;
    assert!(!row_has_content(&before, 0, line as u32 * 16));
    set_line(&terminal, line);
    let after = frame(&terminal, &state);
    assert!(row_has_content(&after, 0, line as u32 * 16), "edited line is stale in tile 0");
}

#[test]
fn aspect_ratio_selection_is_drawn_in_the_tile_containing_the_line() {
    let (terminal, state) = terminal(300, |screen| screen.set_aspect_ratio(true));
    let line = 115;
    let before = frame(&terminal, &state);
    assert!(!row_has_content(&before, 0, line as u32 * 16));
    let mut selection = icy_engine::Selection::new((0, line));
    selection.lead = Position::new(79, line);
    terminal.screen.lock().set_selection(selection).unwrap();
    let after = frame(&terminal, &state);
    assert!(row_has_content(&after, 0, line as u32 * 16), "selection is missing from the cached tile");
}

#[test]
fn edit_on_a_line_that_straddles_two_tiles_repaints_both() {
    // 8x14 fonts do not divide the tile height: line 146 covers raw rows 2044..2058.
    let (terminal, state) = terminal(300, |screen| {
        let font = icy_engine::BitFont::from_sauce_name("IBM EGA").unwrap();
        screen.set_font(0, font);
    });
    let font_height = terminal.screen.lock().font(0).unwrap().size().height as u32;
    assert_eq!(font_height, 14);
    let line = TILE_HEIGHT / font_height;
    assert!(line * font_height < TILE_HEIGHT && (line + 1) * font_height > TILE_HEIGHT);
    let before = frame(&terminal, &state);
    assert!(!row_has_content(&before, 0, TILE_HEIGHT - 1));
    set_line(&terminal, line as i32);
    let after = frame(&terminal, &state);
    assert!(row_has_content(&after, 0, TILE_HEIGHT - 1), "upper part of the line is stale in tile 0");
    assert!(row_has_content(&after, 1, 0), "lower part of the line is missing in tile 1");
}

#[test]
fn skypix_stream_repaints_cached_frames_at_scanline_doubled_size() {
    use icy_engine::{ScreenMode, ScreenSink, TerminalEmulation};
    use icy_engine_gui::util::QueueingSink;
    use std::collections::VecDeque;

    let (screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    let screen: Box<dyn Screen> = screen;
    let state = CRTShaderState::from_screen(&*screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(screen)));
    let initial = frame(&terminal, &state);
    assert_eq!(initial.total_content_height, 400.0);
    assert_eq!(initial.visible_height, 400.0);
    assert_eq!(initial.slices_blink_off[0].height, 400);
    assert!(!row_has_content(&initial, 0, 398));

    for bytes in [&b"Hello SkyPix"[..], b"\x1b[1;639;199!"] {
        let mut queue = VecDeque::new();
        for byte in bytes.chunks(1) {
            parser.parse(byte, &mut QueueingSink::new(&mut queue));
        }
        let mut screen = terminal.screen.lock();
        let mut sink = ScreenSink::new(screen.as_editable().unwrap());
        while let Some(command) = queue.pop_front() {
            command.process_screen_command(&mut sink);
        }
        drop(screen);

        let updated = frame(&terminal, &state);
        assert_ne!(updated.slices_blink_off[0].rgba_data, initial.slices_blink_off[0].rgba_data);
    }

    let final_frame = frame(&terminal, &state);
    assert!(row_has_content(&final_frame, 0, 398));
    assert!(row_has_content(&final_frame, 0, 399));
}

#[test]
fn rip_graphics_text_and_erase_repaint_cached_frames() {
    use icy_engine::{ScreenMode, ScreenSink, TerminalEmulation};
    use icy_engine_gui::util::QueueingSink;
    use icy_parser_core::RipCommand;
    use std::collections::VecDeque;

    let (screen, mut parser) = ScreenMode::Rip.create_screen(TerminalEmulation::Rip, None);
    let screen: Box<dyn Screen> = screen;
    let state = CRTShaderState::from_screen(&*screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(screen)));
    let mut previous = frame(&terminal, &state);
    assert_eq!(previous.total_content_height, 350.0);
    assert_eq!(previous.visible_height, 350.0);
    assert_eq!(previous.slices_blink_off[0].width, 640);
    assert_eq!(previous.slices_blink_off[0].height, 350);
    assert!(!row_has_content(&previous, 0, 349));

    let graphics = format!("!{}{}\n", RipCommand::Color { c: 15 }, RipCommand::Pixel { x: 639, y: 349 });
    for bytes in [graphics.as_bytes(), b"Hello RIP", b"\x1b[2J"] {
        let mut queue = VecDeque::new();
        for byte in bytes.chunks(1) {
            parser.parse(byte, &mut QueueingSink::new(&mut queue));
        }
        let mut screen = terminal.screen.lock();
        let mut sink = ScreenSink::new(screen.as_editable().unwrap());
        while let Some(command) = queue.pop_front() {
            command.process_screen_command(&mut sink);
        }
        drop(screen);

        let updated = frame(&terminal, &state);
        assert_ne!(updated.slices_blink_off[0].rgba_data, previous.slices_blink_off[0].rgba_data, "{bytes:?}");
        previous = updated;
    }
    assert!(!row_has_content(&previous, 0, 349));
}

#[test]
fn skypix_custom_font_changes_preserve_frame_size_and_repaint_glyphs() {
    use icy_engine::{ScreenMode, ScreenSink, TerminalEmulation};
    use icy_engine_gui::util::QueueingSink;
    use std::collections::VecDeque;

    let (screen, mut parser) = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
    let screen: Box<dyn Screen> = screen;
    let state = CRTShaderState::from_screen(&*screen);
    let terminal = Terminal::new(Arc::new(Mutex::new(screen)));
    frame(&terminal, &state);
    for bytes in [
        &b"\x1b[10;19!Sapphire.font!\x1b[19;25;25!Reign of Fire BBS"[..],
        b"\x1b[10;36!Courier.font!\x1b[19;200;100!MW",
        b"\x1b[10;0!\x1b[99!Default font",
    ] {
        let mut queue = VecDeque::new();
        for byte in bytes.chunks(1) {
            parser.parse(byte, &mut QueueingSink::new(&mut queue));
        }
        let mut screen = terminal.screen.lock();
        let mut sink = ScreenSink::new(screen.as_editable().unwrap());
        while let Some(command) = queue.pop_front() {
            command.process_screen_command(&mut sink);
        }
        let expected = screen.render_to_rgba(&Default::default()).1;
        drop(screen);

        let updated = frame(&terminal, &state);
        assert_eq!(updated.total_content_height, 400.0);
        assert_eq!(updated.visible_height, 400.0);
        assert_eq!(updated.slices_blink_off[0].height, 400);
        assert_eq!(*updated.slices_blink_off[0].rgba_data, expected);
    }
}
