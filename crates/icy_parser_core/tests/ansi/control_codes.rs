use super::*;
use icy_parser_core::{AnsiParser, CommandParser, TerminalCommand};

#[test]
fn test_backspace() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x08", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::Backspace);
}

#[test]
fn test_tab() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x09", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::Tab);
}

#[test]
fn test_line_feed() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x0A", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::LineFeed);
}

#[test]
fn test_form_feed() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x0C", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::FormFeed);
}

#[test]
fn test_carriage_return() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x0D", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CarriageReturn);
}

#[test]
fn test_bell() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x07", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::Bell);
}

#[test]
fn test_delete() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x7F", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::Delete);
}

#[test]
fn test_esc_index() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC D - Index (move down one line)
    parser.parse(b"\x1BD", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscIndex);
}

#[test]
fn test_esc_next_line() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC E - Next Line
    parser.parse(b"\x1BE", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscNextLine);
}

#[test]
fn test_esc_set_tab() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC H - Set Tab Stop
    parser.parse(b"\x1BH", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscSetTab);
}

#[test]
fn test_esc_reverse_index() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC M - Reverse Index (move up one line)
    parser.parse(b"\x1BM", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscReverseIndex);
}

#[test]
fn test_esc_save_cursor() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC 7 - Save Cursor Position
    parser.parse(b"\x1B7", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscSaveCursor);
}

#[test]
fn test_esc_restore_cursor() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC 8 - Restore Cursor Position
    parser.parse(b"\x1B8", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscRestoreCursor);
}

#[test]
fn test_esc_reset() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC c - Full Reset
    parser.parse(b"\x1Bc", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscReset);
}
