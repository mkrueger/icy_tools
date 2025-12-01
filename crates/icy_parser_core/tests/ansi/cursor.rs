use super::*;
use icy_parser_core::{AnsiParser, CommandParser, Direction, TerminalCommand, Wrapping};

// Tests for cursor movement and tab operations

#[test]
fn test_csi_cursor_basic_movement() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5A - Cursor Up 5
    parser.parse(b"\x1B[5A", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiMoveCursor(Direction::Up, 5, Wrapping::Never));

    sink.cmds.clear();

    // ESC[B - Cursor Down 1 (default)
    parser.parse(b"\x1B[B", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiMoveCursor(Direction::Down, 1, Wrapping::Never));

    sink.cmds.clear();

    // ESC[3C - Cursor Right 3
    parser.parse(b"\x1B[3C", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiMoveCursor(Direction::Right, 3, Wrapping::Never));

    sink.cmds.clear();

    // ESC[7D - Cursor Left 7
    parser.parse(b"\x1B[7D", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiMoveCursor(Direction::Left, 7, Wrapping::Never));
}

#[test]
fn test_csi_cursor_position() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[10;20H - Cursor Position row 10, col 20
    parser.parse(b"\x1B[10;20H", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiCursorPosition(10, 20));

    sink.cmds.clear();

    // ESC[H - Cursor Position to home (1,1) - default
    parser.parse(b"\x1B[H", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiCursorPosition(1, 1));

    sink.cmds.clear();

    // ESC[5;10f - Alternative cursor position (f instead of H)
    parser.parse(b"\x1B[5;10f", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiCursorPosition(5, 10));
}

#[test]
fn test_csi_cursor_next_previous_line() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[3E - Cursor Next Line (move down 3 lines to column 1)
    parser.parse(b"\x1B[3E", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorNextLine(n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiCursorNextLine(3)");
    }

    sink.cmds.clear();

    // ESC[E - Cursor Next Line (default 1)
    parser.parse(b"\x1B[E", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorNextLine(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiCursorNextLine(1)");
    }

    sink.cmds.clear();

    // ESC[5F - Cursor Previous Line (move up 5 lines to column 1)
    parser.parse(b"\x1B[5F", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorPreviousLine(n) = sink.cmds[0] {
        assert_eq!(n, 5);
    } else {
        panic!("Expected CsiCursorPreviousLine(5)");
    }

    sink.cmds.clear();

    // ESC[F - Cursor Previous Line (default 1)
    parser.parse(b"\x1B[F", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorPreviousLine(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiCursorPreviousLine(1)");
    }
}

#[test]
fn test_csi_cursor_horizontal_absolute() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[20G - Cursor Horizontal Absolute (move to column 20)
    parser.parse(b"\x1B[20G", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorHorizontalAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 20);
    } else {
        panic!("Expected CsiCursorHorizontalAbsolute(20)");
    }

    sink.cmds.clear();

    // ESC[G - Cursor Horizontal Absolute to column 1 (default)
    parser.parse(b"\x1B[G", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorHorizontalAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiCursorHorizontalAbsolute(1)");
    }

    sink.cmds.clear();

    // ESC[80G - Move to column 80
    parser.parse(b"\x1B[80G", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorHorizontalAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 80);
    } else {
        panic!("Expected CsiCursorHorizontalAbsolute(80)");
    }
}

#[test]
fn test_csi_line_position_absolute() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn d - VPA - Line Position Absolute
    parser.parse(b"\x1B[10d", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiLinePositionAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 10);
    } else {
        panic!("Expected CsiLinePositionAbsolute(10)");
    }

    sink.cmds.clear();

    // ESC[d - Line Position Absolute to line 1 (default)
    parser.parse(b"\x1B[d", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiLinePositionAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiLinePositionAbsolute(1)");
    }

    sink.cmds.clear();

    // ESC[24d - Move to line 24
    parser.parse(b"\x1B[24d", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiLinePositionAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 24);
    } else {
        panic!("Expected CsiLinePositionAbsolute(24)");
    }
}

#[test]
fn test_csi_line_position_forward() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn e - VPR - Line Position Forward
    parser.parse(b"\x1B[4e", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiLinePositionForward(n) = sink.cmds[0] {
        assert_eq!(n, 4);
    } else {
        panic!("Expected CsiLinePositionForward(4)");
    }

    sink.cmds.clear();

    // ESC[e - Line Position Forward 1 line (default)
    parser.parse(b"\x1B[e", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiLinePositionForward(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiLinePositionForward(1)");
    }
}

#[test]
fn test_csi_character_position_forward() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn a - HPR - Character Position Forward
    parser.parse(b"\x1B[7a", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCharacterPositionForward(n) = sink.cmds[0] {
        assert_eq!(n, 7);
    } else {
        panic!("Expected CsiCharacterPositionForward(7)");
    }

    sink.cmds.clear();

    // ESC[a - Character Position Forward 1 (default)
    parser.parse(b"\x1B[a", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCharacterPositionForward(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiCharacterPositionForward(1)");
    }
}

#[test]
fn test_csi_horizontal_position_absolute() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn ' - HPA - Horizontal Position Absolute
    parser.parse(b"\x1B[15'", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiHorizontalPositionAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 15);
    } else {
        panic!("Expected CsiHorizontalPositionAbsolute(15)");
    }

    sink.cmds.clear();

    // ESC[1' - Horizontal Position Absolute to column 1 (explicit)
    parser.parse(b"\x1B[1'", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiHorizontalPositionAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiHorizontalPositionAbsolute(1)");
    }
}

#[test]
fn test_cursor_position_aliases() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn j - Character Position Backward (alias for D)
    parser.parse(b"\x1B[5j", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiMoveCursor(Direction::Left, n, Wrapping::Never) = sink.cmds[0] {
        assert_eq!(n, 5);
    } else {
        panic!("Expected CsiMoveCursor(Left, 5)");
    }

    sink.cmds.clear();

    // CSI Pn k - Line Position Backward (alias for A)
    parser.parse(b"\x1B[3k", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiMoveCursor(Direction::Up, n, Wrapping::Never) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiMoveCursor(Up, 3)");
    }
}

#[test]
fn test_save_restore_cursor() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI s - Save Cursor Position
    parser.parse(b"\x1B[s", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiSaveCursorPosition));

    sink.cmds.clear();

    // CSI u - Restore Cursor Position
    parser.parse(b"\x1B[u", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiRestoreCursorPosition));
}

#[test]
fn test_esc_save_restore_cursor() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC 7 - Save Cursor
    parser.parse(b"\x1B7", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscSaveCursor);

    sink.cmds.clear();

    // ESC 8 - Restore Cursor
    parser.parse(b"\x1B8", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscRestoreCursor);
}

// Tab operation tests

#[test]
fn test_csi_clear_tabulation() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Ps g - TBC - Tabulation Clear (clear tab at current position)
    parser.parse(b"\x1B[0g", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiClearTabulation));

    sink.cmds.clear();

    // CSI g - Clear tab at current position (default)
    parser.parse(b"\x1B[g", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiClearTabulation));
}

#[test]
fn test_csi_clear_all_tabs() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI 3g - Clear all tabs
    parser.parse(b"\x1B[3g", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiClearAllTabs));

    sink.cmds.clear();

    // CSI 5g - Also clears all tabs (alternative parameter)
    parser.parse(b"\x1B[5g", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiClearAllTabs));
}

#[test]
fn test_csi_cursor_line_tabulation_forward() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn Y - CVT - Cursor Line Tabulation (forward to next tab)
    parser.parse(b"\x1B[2Y", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorLineTabulationForward(n) = sink.cmds[0] {
        assert_eq!(n, 2);
    } else {
        panic!("Expected CsiCursorLineTabulationForward(2)");
    }

    sink.cmds.clear();

    // CSI Y - Forward 1 tab (default)
    parser.parse(b"\x1B[Y", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorLineTabulationForward(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiCursorLineTabulationForward(1)");
    }
}

#[test]
fn test_csi_cursor_backward_tabulation() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn Z - CBT - Cursor Backward Tabulation
    parser.parse(b"\x1B[3Z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorBackwardTabulation(n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiCursorBackwardTabulation(3)");
    }

    sink.cmds.clear();

    // CSI Z - Backward 1 tab (default)
    parser.parse(b"\x1B[Z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorBackwardTabulation(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiCursorBackwardTabulation(1)");
    }
}

#[test]
fn test_esc_set_tab() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC H - Horizontal Tab Set
    parser.parse(b"\x1BH", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscSetTab);
}
