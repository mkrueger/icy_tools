use super::*;
use icy_parser_core::{AnsiParser, CommandParser, TerminalCommand};

// Tests for rectangular area operations

#[test]
fn test_csi_fill_rectangular_area() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pchar;Pt;Pl;Pb;Pr$x - Fill Rectangular Area
    parser.parse(b"\x1B[65;1;1;10;10$x", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFillRectangularArea {
        char: pchar,
        top,
        left,
        bottom,
        right,
    } = sink.cmds[0]
    {
        assert_eq!(pchar, 65); // 'A'
        assert_eq!(top, 1);
        assert_eq!(left, 1);
        assert_eq!(bottom, 10);
        assert_eq!(right, 10);
    } else {
        panic!("Expected CsiFillRectangularArea");
    }

    sink.cmds.clear();

    // Fill with space character (32)
    parser.parse(b"\x1B[32;5;5;15;20$x", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFillRectangularArea {
        char: pchar,
        top,
        left,
        bottom,
        right,
    } = sink.cmds[0]
    {
        assert_eq!(pchar, 32); // ' '
        assert_eq!(top, 5);
        assert_eq!(left, 5);
        assert_eq!(bottom, 15);
        assert_eq!(right, 20);
    } else {
        panic!("Expected CsiFillRectangularArea");
    }

    sink.cmds.clear();

    // Fill with asterisk (42)
    parser.parse(b"\x1B[42;10;15;20;40$x", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFillRectangularArea {
        char: pchar,
        top,
        left,
        bottom,
        right,
    } = sink.cmds[0]
    {
        assert_eq!(pchar, 42); // '*'
        assert_eq!(top, 10);
        assert_eq!(left, 15);
        assert_eq!(bottom, 20);
        assert_eq!(right, 40);
    } else {
        panic!("Expected CsiFillRectangularArea");
    }

    sink.cmds.clear();

    // Small area
    parser.parse(b"\x1B[88;1;1;2;2$x", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFillRectangularArea {
        char: pchar,
        top,
        left,
        bottom,
        right,
    } = sink.cmds[0]
    {
        assert_eq!(pchar, 88); // 'X'
        assert_eq!(top, 1);
        assert_eq!(left, 1);
        assert_eq!(bottom, 2);
        assert_eq!(right, 2);
    } else {
        panic!("Expected CsiFillRectangularArea");
    }
}

#[test]
fn test_csi_erase_rectangular_area() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pt;Pl;Pb;Pr$z - Erase Rectangular Area
    parser.parse(b"\x1B[5;5;15;20$z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 5);
        assert_eq!(left, 5);
        assert_eq!(bottom, 15);
        assert_eq!(right, 20);
    } else {
        panic!("Expected CsiEraseRectangularArea");
    }

    sink.cmds.clear();

    // Erase entire screen
    parser.parse(b"\x1B[1;1;24;80$z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 1);
        assert_eq!(left, 1);
        assert_eq!(bottom, 24);
        assert_eq!(right, 80);
    } else {
        panic!("Expected CsiEraseRectangularArea");
    }

    sink.cmds.clear();

    // Small rectangle
    parser.parse(b"\x1B[10;20;12;25$z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 10);
        assert_eq!(left, 20);
        assert_eq!(bottom, 12);
        assert_eq!(right, 25);
    } else {
        panic!("Expected CsiEraseRectangularArea");
    }

    sink.cmds.clear();

    // Single line
    parser.parse(b"\x1B[5;10;5;40$z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 5);
        assert_eq!(left, 10);
        assert_eq!(bottom, 5);
        assert_eq!(right, 40);
    } else {
        panic!("Expected CsiEraseRectangularArea");
    }
}

#[test]
fn test_csi_selective_erase_rectangular_area() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pt;Pl;Pb;Pr${ - Selective Erase Rectangular Area
    parser.parse(b"\x1B[2;3;12;18${", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectiveEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 2);
        assert_eq!(left, 3);
        assert_eq!(bottom, 12);
        assert_eq!(right, 18);
    } else {
        panic!("Expected CsiSelectiveEraseRectangularArea");
    }

    sink.cmds.clear();

    // Different area
    parser.parse(b"\x1B[1;1;10;10${", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectiveEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 1);
        assert_eq!(left, 1);
        assert_eq!(bottom, 10);
        assert_eq!(right, 10);
    } else {
        panic!("Expected CsiSelectiveEraseRectangularArea");
    }

    sink.cmds.clear();

    // Large area
    parser.parse(b"\x1B[5;10;20;70${", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectiveEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 5);
        assert_eq!(left, 10);
        assert_eq!(bottom, 20);
        assert_eq!(right, 70);
    } else {
        panic!("Expected CsiSelectiveEraseRectangularArea");
    }

    sink.cmds.clear();

    // Narrow column
    parser.parse(b"\x1B[1;40;24;42${", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectiveEraseRectangularArea { top, left, bottom, right } = sink.cmds[0] {
        assert_eq!(top, 1);
        assert_eq!(left, 40);
        assert_eq!(bottom, 24);
        assert_eq!(right, 42);
    } else {
        panic!("Expected CsiSelectiveEraseRectangularArea");
    }
}
