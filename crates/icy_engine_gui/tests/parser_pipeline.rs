//! Headless integration coverage for syntax parsing, queueing, and real screen execution.
use std::collections::VecDeque;

use icy_engine::{AttributedChar, BufferType, EditableScreen, Position, Screen, ScreenSink, Size, TextAttribute, TextPane, TextScreen};
use icy_engine_gui::util::{QueuedCommand, QueueingSink};
use icy_parser_core::{CommandParser, ViewdataParser};

fn new_screen(width: i32, height: i32, start: Position) -> TextScreen {
    let mut screen = TextScreen::new(Size::new(width, height));
    screen.buffer.buffer_type = BufferType::Viewdata;
    screen.set_caret_position(start);
    screen
}

fn drain(queue: &mut VecDeque<QueuedCommand>, screen: &mut TextScreen) {
    while let Some(command) = queue.pop_front() {
        // Even individual queued commands must survive destruction of the adapter.
        assert!(!command.process_screen_command(&mut ScreenSink::new(screen)));
    }
}

#[derive(Clone, Copy, Debug)]
enum Pipeline {
    Direct,
    Queued,
    QueuedPerChunk,
}

fn execute(chunks: &[&[u8]], width: i32, height: i32, start: Position, pipeline: Pipeline) -> TextScreen {
    let mut screen = new_screen(width, height, start);
    let mut parser = ViewdataParser::new();
    let mut queue = VecDeque::new();
    for chunk in chunks {
        match pipeline {
            Pipeline::Direct => parser.parse(chunk, &mut ScreenSink::new(&mut screen)),
            Pipeline::Queued | Pipeline::QueuedPerChunk => {
                parser.parse(chunk, &mut QueueingSink::new(&mut queue));
                if matches!(pipeline, Pipeline::QueuedPerChunk) {
                    drain(&mut queue, &mut screen);
                }
            }
        }
    }
    drain(&mut queue, &mut screen);
    screen
}

fn assert_same_screen(actual: &TextScreen, expected: &TextScreen, context: &str) {
    assert_eq!(actual.size(), expected.size(), "{context}: size");
    for y in 0..expected.height() {
        for x in 0..expected.width() {
            let pos = Position::new(x, y);
            assert_eq!(actual.char_at(pos), expected.char_at(pos), "{context}: cell {pos:?}");
        }
    }
    assert_eq!(actual.caret_position(), expected.caret_position(), "{context}: cursor");
    assert_eq!(actual.caret().attribute, expected.caret().attribute, "{context}: cursor attributes");
    assert_eq!(actual.caret().visible, expected.caret().visible, "{context}: cursor visibility");
}

/// Compare both execution timing choices, including ESC split from its operand.
/// Concrete assertions on the reference prevent two equally wrong paths passing.
fn assert_chunkings(input: &[u8], width: i32, height: i32, start: Position, check: impl Fn(&TextScreen)) {
    let expected = execute(&[input], width, height, start, Pipeline::Direct);
    check(&expected);
    let mut partitions = vec![vec![input], input.chunks(1).collect::<Vec<_>>()];
    for split in 0..=input.len() {
        partitions.push(vec![&input[..split], &input[split..]]);
    }
    for (partition, chunks) in partitions.iter().enumerate() {
        for pipeline in [Pipeline::Direct, Pipeline::Queued, Pipeline::QueuedPerChunk] {
            let actual = execute(chunks, width, height, start, pipeline);
            assert_same_screen(&actual, &expected, &format!("width={width}, partition={partition}, {pipeline:?}"));
        }
    }
}

fn assert_cell(screen: &TextScreen, x: i32, y: i32, ch: char, foreground: u32, background: u32) {
    assert_eq!(
        screen.char_at(Position::new(x, y)),
        AttributedChar::new(ch, TextAttribute::new(foreground, background)),
        "cell ({x}, {y})"
    );
}

fn held_graphics_row(width: i32) -> Vec<u8> {
    // Color and hold consume one cell each; '!' becomes contiguous mosaic 0x81.
    let mut input = b"\x1bQ\x1b^!".to_vec();
    for _ in 3..width {
        // Changing separation must keep displaying the already-held glyph.
        input.extend_from_slice(b"\x1bZ");
    }
    input.extend_from_slice(b"!\x1b^");
    input
}

#[test]
fn viewdata_graphics_and_hold_reset_on_automatic_wrap() {
    for width in [4, 7] {
        let held = held_graphics_row(width);
        let mut printable = b"\x1bQ\x1b^".to_vec();
        printable.extend(std::iter::repeat_n(b'!', (width - 2) as usize));
        printable.extend_from_slice(b"!\x1b^");
        // Both ordinary printable cells and held control cells can wrap a row.
        for input in [&held, &printable] {
            assert_chunkings(input, width, 3, Position::default(), |screen| {
                assert_cell(screen, 2, 0, '\u{81}', 1, 0);
                assert_cell(screen, width - 1, 0, '\u{81}', 1, 0);
                assert_cell(screen, 0, 1, '!', 7, 0); // Alpha, not a mosaic.
                assert_cell(screen, 1, 1, ' ', 7, 0); // No stale held glyph.
                assert_eq!(screen.caret_position(), Position::new(2, 1));
            });
        }
    }
}

#[test]
fn viewdata_explicit_right_resets_only_when_it_wraps() {
    for width in [4, 7] {
        let mut input = b"\x1bQ\x1b^!".to_vec();
        input.extend(std::iter::repeat_n(b'\t', (width - 3) as usize));
        input.extend_from_slice(b"!\x1b^");
        assert_chunkings(&input, width, 3, Position::default(), |screen| {
            assert_cell(screen, 2, 0, '\u{81}', 1, 0);
            assert_cell(screen, 0, 1, '!', 7, 0);
            assert_cell(screen, 1, 1, ' ', 7, 0);
            assert_eq!(screen.caret_position(), Position::new(2, 1));
        });

        assert_chunkings(b"\x1bQ\t!", width, 3, Position::default(), |screen| {
            assert_cell(screen, 2, 0, '\u{81}', 1, 0);
            assert_eq!(screen.caret_position(), Position::new(3, 0));
        });
    }
}

#[test]
fn viewdata_down_resets_graphics_and_hold_without_changing_column() {
    for width in [4, 7] {
        assert_chunkings(b"\x1bQ\x1b^!\n!", width, 3, Position::default(), |screen| {
            assert_cell(screen, 2, 0, '\u{81}', 1, 0);
            assert_cell(screen, 3, 1, '!', 7, 0);
            let cursor = if width == 4 { Position::new(0, 2) } else { Position::new(4, 1) };
            assert_eq!(screen.caret_position(), cursor);
        });
    }
}

#[test]
fn viewdata_down_resets_all_text_attributes() {
    // Reuse a control cell so even a four-column screen can accumulate attributes.
    let input = b"\x1bA\x08\x1b]\x08\x1bH\x08\x1bM\x08\x1bX\x08!\n!";
    for width in [4, 7] {
        assert_chunkings(input, width, 3, Position::default(), |screen| {
            let before = screen.char_at(Position::new(0, 0));
            assert_eq!(before.ch, '!');
            assert_eq!(before.attribute.foreground(), 1);
            assert_eq!(before.attribute.background(), 1);
            assert!(before.attribute.is_blinking());
            assert!(before.attribute.is_double_height());
            assert!(before.attribute.is_concealed());
            assert_cell(screen, 1, 1, '!', 7, 0);
            assert_eq!(screen.caret_position(), Position::new(2, 1));
        });
    }
}

#[test]
fn viewdata_clear_resets_state_and_cells_but_preserves_hidden_cursor() {
    for width in [4, 7] {
        assert_chunkings(b"\x14\x1bQ\x1b^!\x0c!\x1b^", width, 3, Position::default(), |screen| {
            assert_cell(screen, 0, 0, '!', 7, 0);
            assert_cell(screen, 1, 0, ' ', 7, 0); // Hold must not resurrect the old mosaic.
            for y in 0..screen.height() {
                for x in 0..width {
                    if (x, y) != (0, 0) {
                        // Cleared, unwritten cells need not carry the caret's white foreground.
                        let cell = screen.char_at(Position::new(x, y));
                        assert_eq!(cell.ch, ' ', "clear cell ({x}, {y})");
                        assert_eq!(cell.attribute.background(), 0, "clear background ({x}, {y})");
                    }
                }
            }
            assert_eq!(screen.caret_position(), Position::new(2, 0));
            assert!(!screen.caret().visible);
        });
    }
}

#[test]
fn viewdata_escaped_last_column_does_not_apply_set_after_attributes_to_next_row() {
    for width in [4, 7] {
        for command in [b'R', b'H', b'M', b']'] {
            let mut input = b"\x1bQ\x1b^!\x1b".to_vec();
            input.extend_from_slice(&[command, b'!']);
            assert_chunkings(&input, width, 3, Position::new(width - 4, 0), |screen| {
                assert_cell(screen, width - 2, 0, '\u{81}', 1, 0);
                // Set-before background applies to this cell, but not the next row.
                let background = if command == b']' { 1 } else { 0 };
                assert_cell(screen, width - 1, 0, '\u{81}', 1, background);
                assert_cell(screen, 0, 1, '!', 7, 0);
                assert_eq!(screen.caret_position(), Position::new(1, 1));
            });
        }
    }
}

#[test]
fn viewdata_bottom_row_wraps_to_top_without_scrolling() {
    for width in [4, 7] {
        assert_chunkings(&held_graphics_row(width), width, 2, Position::new(0, 1), |screen| {
            assert_cell(screen, 2, 1, '\u{81}', 1, 0);
            assert_cell(screen, width - 1, 1, '\u{81}', 1, 0);
            assert_cell(screen, 0, 0, '!', 7, 0);
            assert_cell(screen, 1, 0, ' ', 7, 0);
            assert_eq!(screen.caret_position(), Position::new(2, 0));
        });
    }
}

#[test]
fn viewdata_queued_geometry_is_resolved_after_cursor_relocation() {
    let input = b"\x1bQ\x1b^!\x1bR!";
    // One syntax stream can be replayed against different widths and positions.
    let mut queue = VecDeque::new();
    ViewdataParser::new().parse(input, &mut QueueingSink::new(&mut queue));
    for width in [4, 7] {
        let mut screen = new_screen(width, 2, Position::default());
        let start = Position::new(width - 4, 1);
        screen.set_caret_position(start); // Deliberately after queueing the input.
        drain(&mut queue.clone(), &mut screen);
        let expected = execute(&[input], width, 2, start, Pipeline::Direct);
        assert_same_screen(&screen, &expected, "relocated before queue execution");
        assert_cell(&screen, width - 2, 1, '\u{81}', 1, 0);
        assert_cell(&screen, width - 1, 1, '\u{81}', 1, 0);
        assert_cell(&screen, 0, 0, '!', 7, 0);
        assert_eq!(screen.caret_position(), Position::new(1, 0));
    }
}

#[test]
fn viewdata_display_state_follows_terminal_reset() {
    let mut screen = new_screen(7, 3, Position::default());
    let mut parser = ViewdataParser::new();
    parser.parse(b"\x1bQ\x1b^!", &mut ScreenSink::new(&mut screen));
    screen.reset_terminal();
    screen.set_caret_position(Position::default());
    parser.parse(b"!\x1b^", &mut ScreenSink::new(&mut screen));
    assert_eq!(screen.char_at(Position::new(0, 0)).ch, '!');
    assert_eq!(screen.char_at(Position::new(1, 0)).ch, ' ');
}
