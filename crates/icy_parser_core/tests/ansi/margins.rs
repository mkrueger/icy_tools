use super::*;
use icy_parser_core::{AnsiParser, CommandParser, Direction, MarginType};

// Tests for margin and scrolling region commands

#[test]
fn test_reset_margins() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[r - Reset margins (no parameters)
    parser.parse(b"\x1B[r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::ResetMargins);
}

#[test]
fn test_set_top_bottom_margin() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5;20r - Set scrolling region from line 5 to 20
    parser.parse(b"\x1B[5;20r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::SetTopBottomMargin { top: 5, bottom: 20 });
}

#[test]
fn test_csi_set_scrolling_region() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5;20;3r - Set scrolling region with left margin
    parser.parse(b"\x1B[5;20;3r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(
        sink.cmds[0],
        TerminalCommand::CsiSetScrollingRegion {
            top: 5,
            bottom: 20,
            left: 3,
            right: u16::MAX
        }
    );

    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5;20;3;34r - Set scrolling region with all margins
    parser.parse(b"\x1B[5;20;3;34r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(
        sink.cmds[0],
        TerminalCommand::CsiSetScrollingRegion {
            top: 5,
            bottom: 20,
            left: 3,
            right: 34
        }
    );
}

#[test]
fn test_csi_scroll_up_down() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5S - Scroll up 5 lines
    parser.parse(b"\x1B[5S", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Up, n) = sink.cmds[0] {
        assert_eq!(n, 5);
    } else {
        panic!("Expected CsiScroll(Up, 5)");
    }

    sink.cmds.clear();

    // ESC[3T - Scroll down 3 lines
    parser.parse(b"\x1B[3T", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Down, n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiScroll(Down, 3)");
    }

    sink.cmds.clear();

    // ESC[S - Scroll up 1 line (default)
    parser.parse(b"\x1B[S", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Up, n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiScroll(Up, 1)");
    }

    sink.cmds.clear();

    // ESC[T - Scroll down 1 line (default)
    parser.parse(b"\x1B[T", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Down, n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiScroll(Down, 1)");
    }
}

#[test]
fn test_csi_scroll_left_right() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn A - Scroll Right (with space intermediate)
    parser.parse(b"\x1B[4 A", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Right, n) = sink.cmds[0] {
        assert_eq!(n, 4);
    } else {
        panic!("Expected CsiScroll(Right, 4)");
    }

    sink.cmds.clear();

    // CSI Pn @ - Scroll Left (with space intermediate)
    parser.parse(b"\x1B[3 @", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Left, n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiScroll(Left, 3)");
    }
}

#[test]
fn test_csi_equals_set_specific_margins() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[=0;5m - Set top margin to 5
    parser.parse(b"\x1B[=0;5m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEqualsSetSpecificMargins(margin_type, value) = sink.cmds[0] {
        assert_eq!(margin_type, MarginType::Top);
        assert_eq!(value, 5);
    } else {
        panic!("Expected CsiEqualsSetSpecificMargins(Top, 5)");
    }

    sink.cmds.clear();

    // ESC[=1;20m - Set bottom margin to 20
    parser.parse(b"\x1B[=1;20m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEqualsSetSpecificMargins(margin_type, value) = sink.cmds[0] {
        assert_eq!(margin_type, MarginType::Bottom);
        assert_eq!(value, 20);
    } else {
        panic!("Expected CsiEqualsSetSpecificMargins(Bottom, 20)");
    }

    sink.cmds.clear();

    // ESC[=2;3m - Set left margin to 3
    parser.parse(b"\x1B[=2;3m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEqualsSetSpecificMargins(margin_type, value) = sink.cmds[0] {
        assert_eq!(margin_type, MarginType::Left);
        assert_eq!(value, 3);
    } else {
        panic!("Expected CsiEqualsSetSpecificMargins(Left, 3)");
    }

    sink.cmds.clear();

    // ESC[=3;80m - Set right margin to 80
    parser.parse(b"\x1B[=3;80m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEqualsSetSpecificMargins(margin_type, value) = sink.cmds[0] {
        assert_eq!(margin_type, MarginType::Right);
        assert_eq!(value, 80);
    } else {
        panic!("Expected CsiEqualsSetSpecificMargins(Right, 80)");
    }
}

#[test]
fn test_reset_left_and_right_margin() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5;80s - Reset left and right margins
    parser.parse(b"\x1B[5;80s", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::ResetLeftAndRightMargin { left, right } = sink.cmds[0] {
        assert_eq!(left, 5);
        assert_eq!(right, 80);
    } else {
        panic!("Expected ResetLeftAndRightMargin {{ left: 5, right: 80 }}");
    }

    sink.cmds.clear();

    // ESC[1;132s - Reset to full width
    parser.parse(b"\x1B[1;132s", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::ResetLeftAndRightMargin { left, right } = sink.cmds[0] {
        assert_eq!(left, 1);
        assert_eq!(right, 132);
    } else {
        panic!("Expected ResetLeftAndRightMargin {{ left: 1, right: 132 }}");
    }
}

#[test]
fn test_scroll_area() {
    let mut sink = CollectSink::new();

    // Scroll area is typically generated by Avatar parser, but can be tested directly
    // Note: This would require implementing parsing for scroll area if not already present
    // For now, we'll test by emitting directly in the sink

    // Create a scroll area command and verify structure
    let cmd = TerminalCommand::ScrollArea {
        direction: Direction::Up,
        num_lines: 3,
        top: 5,
        left: 10,
        bottom: 20,
        right: 70,
    };

    sink.emit(cmd.clone());
    assert_eq!(sink.cmds.len(), 1);

    if let TerminalCommand::ScrollArea {
        direction,
        num_lines,
        top,
        left,
        bottom,
        right,
    } = sink.cmds[0]
    {
        assert_eq!(direction, Direction::Up);
        assert_eq!(num_lines, 3);
        assert_eq!(top, 5);
        assert_eq!(left, 10);
        assert_eq!(bottom, 20);
        assert_eq!(right, 70);
    } else {
        panic!("Expected ScrollArea command");
    }

    sink.cmds.clear();

    // Test scroll down direction
    let cmd = TerminalCommand::ScrollArea {
        direction: Direction::Down,
        num_lines: 5,
        top: 1,
        left: 1,
        bottom: 24,
        right: 80,
    };

    sink.emit(cmd);
    assert_eq!(sink.cmds.len(), 1);

    if let TerminalCommand::ScrollArea {
        direction,
        num_lines,
        top,
        left,
        bottom,
        right,
    } = sink.cmds[0]
    {
        assert_eq!(direction, Direction::Down);
        assert_eq!(num_lines, 5);
        assert_eq!(top, 1);
        assert_eq!(left, 1);
        assert_eq!(bottom, 24);
        assert_eq!(right, 80);
    } else {
        panic!("Expected ScrollArea command");
    }
}
