use super::*;
use icy_parser_core::{AnsiParser, BaudEmulation, CaretShape, CommandParser, CommunicationLine, SpecialKey, TerminalCommand};

// Tests for terminal operations: resize, special keys, caret style, fonts, communication speed

#[test]
fn test_csi_resize_terminal() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI 8;{height};{width}t - Resize Terminal
    parser.parse(b"\x1B[8;24;80t", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiResizeTerminal(height, width) = sink.cmds[0] {
        assert_eq!(height, 24);
        assert_eq!(width, 80);
    } else {
        panic!("Expected CsiResizeTerminal(24, 80)");
    }

    sink.cmds.clear();

    // CSI 8;{height};{width}t - Different dimensions
    parser.parse(b"\x1B[8;50;132t", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiResizeTerminal(height, width) = sink.cmds[0] {
        assert_eq!(height, 50);
        assert_eq!(width, 132);
    } else {
        panic!("Expected CsiResizeTerminal(50, 132)");
    }

    sink.cmds.clear();

    // CSI 8;{height};{width}t - Small terminal
    parser.parse(b"\x1B[8;10;40t", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiResizeTerminal(height, width) = sink.cmds[0] {
        assert_eq!(height, 10);
        assert_eq!(width, 40);
    } else {
        panic!("Expected CsiResizeTerminal(10, 40)");
    }
}

#[test]
fn test_csi_special_key() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI 2 ~ - Insert
    parser.parse(b"\x1B[2~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::Insert);
    } else {
        panic!("Expected CsiSpecialKey(Insert)");
    }

    sink.cmds.clear();

    // CSI 3 ~ - Delete
    parser.parse(b"\x1B[3~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::Delete);
    } else {
        panic!("Expected CsiSpecialKey(Delete)");
    }

    sink.cmds.clear();

    // CSI 5 ~ - Page Up
    parser.parse(b"\x1B[5~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::PageUp);
    } else {
        panic!("Expected CsiSpecialKey(PageUp)");
    }

    sink.cmds.clear();

    // CSI 6 ~ - Page Down
    parser.parse(b"\x1B[6~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::PageDown);
    } else {
        panic!("Expected CsiSpecialKey(PageDown)");
    }

    sink.cmds.clear();

    // CSI 7 ~ - Home
    parser.parse(b"\x1B[7~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::Home);
    } else {
        panic!("Expected CsiSpecialKey(Home)");
    }

    sink.cmds.clear();

    // CSI 8 ~ - End
    parser.parse(b"\x1B[8~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::End);
    } else {
        panic!("Expected CsiSpecialKey(End)");
    }

    sink.cmds.clear();

    // CSI 11 ~ - F1
    parser.parse(b"\x1B[11~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F1);
    } else {
        panic!("Expected CsiSpecialKey(F1)");
    }

    sink.cmds.clear();

    // CSI 12 ~ - F2
    parser.parse(b"\x1B[12~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F2);
    } else {
        panic!("Expected CsiSpecialKey(F2)");
    }

    sink.cmds.clear();

    // CSI 13 ~ - F3
    parser.parse(b"\x1B[13~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F3);
    } else {
        panic!("Expected CsiSpecialKey(F3)");
    }

    sink.cmds.clear();

    // CSI 14 ~ - F4
    parser.parse(b"\x1B[14~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F4);
    } else {
        panic!("Expected CsiSpecialKey(F4)");
    }

    sink.cmds.clear();

    // CSI 15 ~ - F5
    parser.parse(b"\x1B[15~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F5);
    } else {
        panic!("Expected CsiSpecialKey(F5)");
    }

    sink.cmds.clear();

    // CSI 17 ~ - F6
    parser.parse(b"\x1B[17~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F6);
    } else {
        panic!("Expected CsiSpecialKey(F6)");
    }

    sink.cmds.clear();

    // CSI 18 ~ - F7
    parser.parse(b"\x1B[18~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F7);
    } else {
        panic!("Expected CsiSpecialKey(F7)");
    }

    sink.cmds.clear();

    // CSI 19 ~ - F8
    parser.parse(b"\x1B[19~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F8);
    } else {
        panic!("Expected CsiSpecialKey(F8)");
    }

    sink.cmds.clear();

    // CSI 20 ~ - F9
    parser.parse(b"\x1B[20~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F9);
    } else {
        panic!("Expected CsiSpecialKey(F9)");
    }

    sink.cmds.clear();

    // CSI 21 ~ - F10
    parser.parse(b"\x1B[21~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F10);
    } else {
        panic!("Expected CsiSpecialKey(F10)");
    }

    sink.cmds.clear();

    // CSI 23 ~ - F11
    parser.parse(b"\x1B[23~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F11);
    } else {
        panic!("Expected CsiSpecialKey(F11)");
    }

    sink.cmds.clear();

    // CSI 24 ~ - F12
    parser.parse(b"\x1B[24~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(key) = sink.cmds[0] {
        assert_eq!(key, SpecialKey::F12);
    } else {
        panic!("Expected CsiSpecialKey(F12)");
    }
}

#[test]
fn test_special_key_to_sequence() {
    // Test that to_sequence() generates the correct ANSI escape sequences
    assert_eq!(SpecialKey::Insert.to_sequence(), "\x1B[2~");
    assert_eq!(SpecialKey::Delete.to_sequence(), "\x1B[3~");
    assert_eq!(SpecialKey::PageUp.to_sequence(), "\x1B[5~");
    assert_eq!(SpecialKey::PageDown.to_sequence(), "\x1B[6~");
    assert_eq!(SpecialKey::Home.to_sequence(), "\x1B[7~");
    assert_eq!(SpecialKey::End.to_sequence(), "\x1B[8~");
    assert_eq!(SpecialKey::F1.to_sequence(), "\x1B[11~");
    assert_eq!(SpecialKey::F2.to_sequence(), "\x1B[12~");
    assert_eq!(SpecialKey::F3.to_sequence(), "\x1B[13~");
    assert_eq!(SpecialKey::F4.to_sequence(), "\x1B[14~");
    assert_eq!(SpecialKey::F5.to_sequence(), "\x1B[15~");
    assert_eq!(SpecialKey::F6.to_sequence(), "\x1B[17~");
    assert_eq!(SpecialKey::F7.to_sequence(), "\x1B[18~");
    assert_eq!(SpecialKey::F8.to_sequence(), "\x1B[19~");
    assert_eq!(SpecialKey::F9.to_sequence(), "\x1B[20~");
    assert_eq!(SpecialKey::F10.to_sequence(), "\x1B[21~");
    assert_eq!(SpecialKey::F11.to_sequence(), "\x1B[23~");
    assert_eq!(SpecialKey::F12.to_sequence(), "\x1B[24~");
}

#[test]
fn test_csi_set_caret_style() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI 0 q - Default: blinking block
    parser.parse(b"\x1B[0 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, true);
        assert_eq!(shape, CaretShape::Block);
    } else {
        panic!("Expected CsiSetCaretStyle(true, Block)");
    }

    sink.cmds.clear();

    // CSI 1 q - Blinking block
    parser.parse(b"\x1B[1 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, true);
        assert_eq!(shape, CaretShape::Block);
    } else {
        panic!("Expected CsiSetCaretStyle(true, Block)");
    }

    sink.cmds.clear();

    // CSI 2 q - Steady block
    parser.parse(b"\x1B[2 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, false);
        assert_eq!(shape, CaretShape::Block);
    } else {
        panic!("Expected CsiSetCaretStyle(false, Block)");
    }

    sink.cmds.clear();

    // CSI 3 q - Blinking underline
    parser.parse(b"\x1B[3 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, true);
        assert_eq!(shape, CaretShape::Underline);
    } else {
        panic!("Expected CsiSetCaretStyle(true, Underline)");
    }

    sink.cmds.clear();

    // CSI 4 q - Steady underline
    parser.parse(b"\x1B[4 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, false);
        assert_eq!(shape, CaretShape::Underline);
    } else {
        panic!("Expected CsiSetCaretStyle(false, Underline)");
    }

    sink.cmds.clear();

    // CSI 5 q - Blinking bar
    parser.parse(b"\x1B[5 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, true);
        assert_eq!(shape, CaretShape::Bar);
    } else {
        panic!("Expected CsiSetCaretStyle(true, Bar)");
    }

    sink.cmds.clear();

    // CSI 6 q - Steady bar
    parser.parse(b"\x1B[6 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, false);
        assert_eq!(shape, CaretShape::Bar);
    } else {
        panic!("Expected CsiSetCaretStyle(false, Bar)");
    }
}

#[test]
fn test_csi_font_selection() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Ps1;Ps2 D - Font Selection (with space intermediate)
    parser.parse(b"\x1B[1;5 D", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFontSelection { slot, font_number } = sink.cmds[0] {
        assert_eq!(slot, 1);
        assert_eq!(font_number, 5);
    } else {
        panic!("Expected CsiFontSelection {{ slot: 1, font_number: 5 }}");
    }

    sink.cmds.clear();

    // Different slot and font
    parser.parse(b"\x1B[0;3 D", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFontSelection { slot, font_number } = sink.cmds[0] {
        assert_eq!(slot, 0);
        assert_eq!(font_number, 3);
    } else {
        panic!("Expected CsiFontSelection {{ slot: 0, font_number: 3 }}");
    }

    sink.cmds.clear();

    // Slot 2, font 7
    parser.parse(b"\x1B[2;7 D", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFontSelection { slot, font_number } = sink.cmds[0] {
        assert_eq!(slot, 2);
        assert_eq!(font_number, 7);
    } else {
        panic!("Expected CsiFontSelection {{ slot: 2, font_number: 7 }}");
    }
}

#[test]
fn test_set_font_page() {
    let mut sink = CollectSink::new();

    // SetFontPage is typically set directly via API, not parsed
    // But we can test the command structure
    sink.emit(TerminalCommand::SetFontPage(0));
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::SetFontPage(page) = sink.cmds[0] {
        assert_eq!(page, 0);
    } else {
        panic!("Expected SetFontPage(0)");
    }

    sink.cmds.clear();

    sink.emit(TerminalCommand::SetFontPage(1));
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::SetFontPage(page) = sink.cmds[0] {
        assert_eq!(page, 1);
    } else {
        panic!("Expected SetFontPage(1)");
    }

    sink.cmds.clear();

    sink.emit(TerminalCommand::SetFontPage(128));
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::SetFontPage(page) = sink.cmds[0] {
        assert_eq!(page, 128);
    } else {
        panic!("Expected SetFontPage(128)");
    }
}

#[test]
fn test_csi_select_communication_speed() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Ps1;Ps2*r - Select Communication Speed
    // Ps1 = communication line, Ps2 = baud rate index

    // Host Transmit, 9600 baud
    parser.parse(b"\x1B[0;6*r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectCommunicationSpeed(comm_line, baud) = sink.cmds[0] {
        assert_eq!(comm_line, CommunicationLine::HostTransmit);
        assert_eq!(baud, BaudEmulation::Rate(9600));
    } else {
        panic!("Expected CsiSelectCommunicationSpeed(HostTransmit, 9600)");
    }

    sink.cmds.clear();

    // Host Receive, 9600 baud
    parser.parse(b"\x1B[2;6*r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectCommunicationSpeed(comm_line, baud) = sink.cmds[0] {
        assert_eq!(comm_line, CommunicationLine::HostReceive);
        assert_eq!(baud, BaudEmulation::Rate(9600));
    } else {
        panic!("Expected CsiSelectCommunicationSpeed(HostReceive, 9600)");
    }

    sink.cmds.clear();

    // Printer, 1200 baud
    parser.parse(b"\x1B[3;3*r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectCommunicationSpeed(comm_line, baud) = sink.cmds[0] {
        assert_eq!(comm_line, CommunicationLine::Printer);
        assert_eq!(baud, BaudEmulation::Rate(1200));
    } else {
        panic!("Expected CsiSelectCommunicationSpeed(Printer, 1200)");
    }

    sink.cmds.clear();

    // Host Transmit, 300 baud
    parser.parse(b"\x1B[1;1*r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectCommunicationSpeed(comm_line, baud) = sink.cmds[0] {
        assert_eq!(comm_line, CommunicationLine::HostTransmit);
        assert_eq!(baud, BaudEmulation::Rate(300));
    } else {
        panic!("Expected CsiSelectCommunicationSpeed(HostTransmit, 300)");
    }
}
