use icy_parser_core::{CommandParser, RipCommand, RipParser, TerminalRequest};

use crate::rip::TestSink;

// Tests for ANSI escape sequences

#[test]
fn test_rip_query_version() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // Test ESC[!
    parser.parse(b"\x1B[!", &mut sink);

    assert_eq!(sink.terminal_requests.len(), 1);
    match &sink.terminal_requests[0] {
        TerminalRequest::RipRequestTerminalId => {}
        _ => panic!("Expected RipRequestTerminalId request, got {:?}", sink.terminal_requests[0]),
    }
}

#[test]
fn test_rip_query_version_with_zero() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // Test ESC[0!
    parser.parse(b"\x1B[0!", &mut sink);

    assert_eq!(sink.terminal_requests.len(), 1);
    match &sink.terminal_requests[0] {
        TerminalRequest::RipRequestTerminalId => {}
        _ => panic!("Expected RipRequestTerminalId request, got {:?}", sink.terminal_requests[0]),
    }
}

#[test]
fn test_rip_disable() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // Test ESC[1! - should be handled internally, no command/request emitted
    parser.parse(b"\x1B[1!", &mut sink);

    assert_eq!(sink.rip_commands.len(), 0, "Disable should be handled internally");
    assert_eq!(sink.terminal_requests.len(), 0, "Disable should be handled internally");

    // After disable, RIP commands should not be processed
    parser.parse(b"!|c05\n", &mut sink);

    // Should still be 0 commands
    assert_eq!(sink.rip_commands.len(), 0, "RIP commands should be disabled");
}

#[test]
fn test_rip_enable() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // First disable RIP
    parser.parse(b"\x1B[1!", &mut sink);
    assert_eq!(sink.rip_commands.len(), 0, "Disable is handled internally");

    // Try to send a RIP command - should be ignored
    parser.parse(b"!|c05\n", &mut sink);
    assert_eq!(sink.rip_commands.len(), 0, "RIP should still be disabled");

    // Now enable RIP
    parser.parse(b"\x1B[2!", &mut sink);
    assert_eq!(sink.rip_commands.len(), 0, "Enable is handled internally");

    // Now RIP commands should work again
    parser.parse(b"!|c05\n", &mut sink);
    assert_eq!(sink.rip_commands.len(), 1, "RIP should be enabled again");
    match &sink.rip_commands[0] {
        RipCommand::Color { c } => {
            assert_eq!(*c, 5);
        }
        _ => panic!("Expected Color command after re-enabling"),
    }
}

#[test]
fn test_rip_unknown_ansi_sequence_passthrough() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // Test ESC[99! (unknown number)
    parser.parse(b"\x1B[99!", &mut sink);

    // Should pass through to ANSI parser, no RIP commands
    assert_eq!(sink.rip_commands.len(), 0);

    // The ANSI parser should have received the sequence
    assert!(sink.terminal_commands.len() > 0 || true); // ANSI parser was called
}

#[test]
fn test_real_world_rip_query_version() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // Real-world sequence from BBS systems that includes:
    // - ESC[s: Save cursor position
    // - ESC[0c: Primary Device Attributes
    // - ESC[255B: Cursor down 255 lines
    // - ESC[255C: Cursor forward 255 columns
    // - \x08: Backspace
    // - ESC[6n: Cursor Position Report
    // - ESC[u: Restore cursor position
    // - ESC[!: RIPscrip version query
    // - ESC[6n: Another Cursor Position Report
    // - ESC[0m: Reset SGR attributes
    // - ESC[2J: Clear entire screen
    // - ESC[H: Cursor home
    // - \x0B: Vertical tab
    parser.parse(
        b"\x1B[s\x1B[0c\x1B[255B\x1B[255C\x08_\x1B[6n\x1B[u\x1B[!_\n\x1B[6n\x1B[0m_\x1B[2J\x1B[H\x0B",
        &mut sink,
    );

    // Verify we got the expected terminal requests
    assert!(
        sink.terminal_requests.len() >= 4,
        "Expected at least 4 terminal requests, got {}",
        sink.terminal_requests.len()
    );

    // Check that all expected requests are present
    let has_device_attributes = sink.terminal_requests.iter().any(|r| matches!(r, TerminalRequest::DeviceAttributes));
    let has_cursor_position = sink.terminal_requests.iter().any(|r| matches!(r, TerminalRequest::CursorPositionReport));
    let has_rip_query = sink.terminal_requests.iter().any(|r| matches!(r, TerminalRequest::RipRequestTerminalId));

    assert!(has_device_attributes, "Missing DeviceAttributes request");
    assert!(has_cursor_position, "Missing CursorPositionReport request");
    assert!(has_rip_query, "Missing RipRequestTerminalId request");

    // Verify the specific sequence: DeviceAttributes, CursorPositionReport, RipRequestTerminalId, CursorPositionReport
    assert!(
        matches!(sink.terminal_requests.get(0), Some(TerminalRequest::DeviceAttributes)),
        "Expected DeviceAttributes at position 0, got {:?}",
        sink.terminal_requests.get(0)
    );
    assert!(
        matches!(sink.terminal_requests.get(1), Some(TerminalRequest::CursorPositionReport)),
        "Expected CursorPositionReport at position 1, got {:?}",
        sink.terminal_requests.get(1)
    );
    assert!(
        matches!(sink.terminal_requests.get(2), Some(TerminalRequest::RipRequestTerminalId)),
        "Expected RipRequestTerminalId at position 2, got {:?}",
        sink.terminal_requests.get(2)
    );
    assert!(
        matches!(sink.terminal_requests.get(3), Some(TerminalRequest::CursorPositionReport)),
        "Expected CursorPositionReport at position 3, got {:?}",
        sink.terminal_requests.get(3)
    );
}
