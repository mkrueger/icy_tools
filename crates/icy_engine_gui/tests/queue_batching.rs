use std::collections::VecDeque;

use icy_engine::{BufferType, Screen, ScreenSink, Size, TextPane, TextScreen};
use icy_engine_gui::util::{QueuedCommand, QueueingSink};
use icy_parser_core::{AnsiParser, CommandParser, CommandSink, OperatingSystemCommand, TerminalCommand, TerminalRequest};

#[test]
fn tiny_prints_coalesce_across_sink_lifetimes() {
    let mut queue = VecDeque::new();
    for byte in b"Hello World" {
        QueueingSink::new(&mut queue).print(&[*byte]);
    }
    QueueingSink::new(&mut queue).print(b"");
    assert_eq!(queue.len(), 1);
    assert!(matches!(&queue[0], QueuedCommand::Print(text) if text == b"Hello World"));
}

#[test]
fn empty_print_does_not_create_an_event() {
    let mut queue = VecDeque::new();
    QueueingSink::new(&mut queue).print(b"");
    assert!(queue.is_empty());
}

#[test]
fn non_text_events_are_ordering_barriers() {
    let mut queue = VecDeque::new();
    let mut sink = QueueingSink::new(&mut queue);
    sink.print(b"A");
    sink.emit(TerminalCommand::CarriageReturn);
    sink.print(b"B");
    sink.request(TerminalRequest::CursorPositionReport);
    sink.print(b"C");
    sink.operating_system_command(OperatingSystemCommand::SetTitle(b"title".to_vec()));
    sink.print(b"D");
    sink.aps(b"payload");
    sink.print(b"E");
    assert_eq!(queue.len(), 9);
    for (index, expected) in b"ABCDE".iter().enumerate() {
        assert!(matches!(&queue[index * 2], QueuedCommand::Print(text) if text == &[*expected]));
    }
    assert!(matches!(queue[1], QueuedCommand::Command(TerminalCommand::CarriageReturn)));
    assert!(matches!(queue[3], QueuedCommand::TerminalRequest(TerminalRequest::CursorPositionReport)));
    assert!(matches!(&queue[5], QueuedCommand::OperatingSystemCommand(OperatingSystemCommand::SetTitle(text)) if text == b"title"));
    assert!(matches!(&queue[7], QueuedCommand::Aps(text) if text == b"payload"));
}

#[test]
fn text_blocks_are_bounded_without_losing_bytes() {
    let limit = QueueingSink::MAX_PRINT_BYTES;
    let input: Vec<u8> = (0..(limit * 3 + 17)).map(|n| n as u8).collect();
    for chunk_size in [1, 63, limit - 1, limit, input.len()] {
        let mut queue = VecDeque::new();
        for chunk in input.chunks(chunk_size) {
            QueueingSink::new(&mut queue).print(chunk);
        }
        assert_eq!(queue.len(), input.len().div_ceil(limit));
        let mut result = Vec::new();
        for command in queue {
            let QueuedCommand::Print(bytes) = command else {
                panic!("unexpected non-text event")
            };
            assert!(!bytes.is_empty() && bytes.len() <= limit);
            result.extend_from_slice(&bytes);
        }
        assert_eq!(result, input);
    }
}

#[test]
fn ansi_screen_output_is_independent_of_queue_batching_and_utf8_splits() {
    let input = "Aäα😀\x1b[31mRed\x1b[0m\r\nNext\x1b[2;3H!".as_bytes();
    let make_screen = || {
        let mut screen = TextScreen::new(Size::new(40, 4));
        screen.buffer.buffer_type = BufferType::Unicode;
        screen
    };
    let mut expected = make_screen();
    AnsiParser::new().parse(input, &mut ScreenSink::new(&mut expected));
    for chunk_size in [1, 2, 7, input.len()] {
        for drain_per_chunk in [false, true] {
            let mut screen = make_screen();
            let mut queue = VecDeque::new();
            let mut parser = AnsiParser::new();
            for chunk in input.chunks(chunk_size) {
                parser.parse(chunk, &mut QueueingSink::new(&mut queue));
                if drain_per_chunk {
                    while let Some(command) = queue.pop_front() {
                        command.process_screen_command(&mut ScreenSink::new(&mut screen));
                    }
                }
            }
            while let Some(command) = queue.pop_front() {
                command.process_screen_command(&mut ScreenSink::new(&mut screen));
            }
            for y in 0..4 {
                for x in 0..40 {
                    assert_eq!(screen.char_at((x, y).into()), expected.char_at((x, y).into()));
                }
            }
            assert_eq!(screen.caret_position(), expected.caret_position());
            assert_eq!(screen.caret().attribute, expected.caret().attribute);
        }
    }
    assert_eq!(expected.char_at((1, 0).into()).ch, 'ä');
    assert_eq!(expected.char_at((2, 0).into()).ch, 'α');
    assert_eq!(expected.char_at((3, 0).into()).ch, '😀');
}
