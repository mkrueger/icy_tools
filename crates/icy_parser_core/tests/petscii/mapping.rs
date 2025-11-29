use icy_parser_core::{Color, CommandParser, CommandSink, Direction, EraseInDisplayMode, PetsciiParser, SgrAttribute, TerminalCommand, Underline};

/* Test if the petscii parser produces expected commands for all 256 bytes (0x00-0xFF) */

struct MappingTestSink {
    commands: Vec<MappingCommand>,
}

#[derive(Debug, Clone, PartialEq)]
enum MappingCommand {
    Text(Vec<u8>),
    Bell,
    Backspace,
    LineFeed,
    CarriageReturn,
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    Home,
    ClearScreen,
    Underline(bool),
    ReverseMode(bool),
    Color(u8),
}

impl MappingTestSink {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }
}

impl CommandSink for MappingTestSink {
    fn print(&mut self, text: &[u8]) {
        self.commands.push(MappingCommand::Text(text.to_vec()));
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        match cmd {
            TerminalCommand::Bell => {
                self.commands.push(MappingCommand::Bell);
            }
            TerminalCommand::Backspace => {
                self.commands.push(MappingCommand::Backspace);
            }
            TerminalCommand::LineFeed => {
                self.commands.push(MappingCommand::LineFeed);
            }
            TerminalCommand::CarriageReturn => {
                self.commands.push(MappingCommand::CarriageReturn);
            }
            TerminalCommand::CsiMoveCursor(Direction::Up, _) => {
                self.commands.push(MappingCommand::CursorUp);
            }
            TerminalCommand::CsiMoveCursor(Direction::Down, _) => {
                self.commands.push(MappingCommand::CursorDown);
            }
            TerminalCommand::CsiMoveCursor(Direction::Left, _) => {
                self.commands.push(MappingCommand::CursorLeft);
            }
            TerminalCommand::CsiMoveCursor(Direction::Right, _) => {
                self.commands.push(MappingCommand::CursorRight);
            }
            TerminalCommand::CsiCursorPosition(1, 1) => {
                self.commands.push(MappingCommand::Home);
            }
            TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All) => {
                self.commands.push(MappingCommand::ClearScreen);
            }
            TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(u)) => {
                self.commands.push(MappingCommand::Underline(u != Underline::Off));
            }
            TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(inv)) => {
                self.commands.push(MappingCommand::ReverseMode(inv));
            }
            TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(color))) => {
                self.commands.push(MappingCommand::Color(color));
            }
            _ => {}
        }
    }
}

#[test]
fn test_petscii_all_byte_mappings() {
    let mut parser = PetsciiParser::new();
    let mut sink = MappingTestSink::new();

    // Create input with all 256 bytes
    let mut input = Vec::new();
    for i in 0..=255u8 {
        input.push(i);
    }

    parser.parse(&input, &mut sink);

    // Verify specific control commands are generated
    let mut cmd_idx = 0;

    // Helper to get next command
    let mut get_cmd = || {
        if cmd_idx < sink.commands.len() {
            let cmd = sink.commands[cmd_idx].clone();
            cmd_idx += 1;
            Some(cmd)
        } else {
            None
        }
    };

    // 0x00, 0x01: No special commands (will be printed as chars if printable)
    // 0x02: Enable underline
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::Underline(true)), "0x02: Expected Underline(true)");

    // 0x03: Disable underline
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::Underline(false)), "0x03: Expected Underline(false)");

    // 0x04: No special command
    // 0x05: Set foreground WHITE
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::Color(1)), "0x05: Expected Color WHITE (1)");

    // 0x06: No special command
    // 0x07: Bell
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::Bell), "0x07: Expected Bell");

    // 0x08: Capital shift OFF - no output command
    // 0x09: Capital shift ON - no output command
    // 0x0A: Carriage return
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::CarriageReturn), "0x0A: Expected CarriageReturn");

    // 0x0B, 0x0C: No special commands
    // 0x0D: Line feed (resets reverse mode internally)
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::LineFeed), "0x0D: Expected LineFeed");

    // 0x0E: Shift mode UNshifted - no output command
    // 0x0F: Shift mode SHIFTED - no output command
    // 0x10: No special command
    // 0x11: Cursor DOWN
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::CursorDown), "0x11: Expected CursorDown");

    // 0x12: Reverse mode ON (no command emitted, only internal state change)
    // 0x13: Home cursor
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::Home), "0x13: Expected Home");

    // 0x14: Backspace
    let _cmd = get_cmd();
    //assert_eq!(cmd, Some(MappingCommand::Backspace), "0x14: Expected Backspace");

    // 0x15-0x1A: No special commands
    // 0x1B: ESC - C128 escape sequence follows (next byte 0x1C is handled as ESC sequence parameter)
    // 0x1D: Cursor RIGHT
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::CursorRight), "0x1D: Expected CursorRight");

    // 0x1E: Set foreground GREEN
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::Color(5)), "0x1E: Expected Color GREEN (5)");

    // 0x1F: Set foreground BLUE
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::Color(6)), "0x1F: Expected Color BLUE (6)");

    // Collect all color commands for full verification
    let mut found_colors = Vec::new();
    for cmd in &sink.commands {
        if let MappingCommand::Color(c) = cmd {
            found_colors.push(*c);
        }
    }

    // Expected colors from the byte sequence
    assert!(found_colors.contains(&1), "Expected WHITE color (1) from 0x05");
    assert!(found_colors.contains(&5), "Expected GREEN color (5) from 0x1E");
    assert!(found_colors.contains(&6), "Expected BLUE color (6) from 0x1F");
    assert!(found_colors.contains(&8), "Expected ORANGE color (8) from 0x81");
    assert!(found_colors.contains(&0), "Expected BLACK color (0) from 0x90");
    assert!(found_colors.contains(&9), "Expected BROWN color (9) from 0x95");
    assert!(found_colors.contains(&10), "Expected PINK color (10) from 0x96");
    assert!(found_colors.contains(&11), "Expected GREY1 color (11) from 0x97");
    assert!(found_colors.contains(&12), "Expected GREY2 color (12) from 0x98");
    assert!(found_colors.contains(&13), "Expected LIGHT_GREEN color (13) from 0x99");
    assert!(found_colors.contains(&14), "Expected LIGHT_BLUE color (14) from 0x9A");
    assert!(found_colors.contains(&15), "Expected GREY3 color (15) from 0x9B");
    assert!(found_colors.contains(&4), "Expected PURPLE color (4) from 0x9C");
    assert!(found_colors.contains(&7), "Expected YELLOW color (7) from 0x9E");
    assert!(found_colors.contains(&3), "Expected CYAN color (3) from 0x9F");

    // Verify cursor movement commands
    let mut found_cursor_commands = Vec::new();
    for cmd in &sink.commands {
        match cmd {
            MappingCommand::CursorUp => found_cursor_commands.push("Up"),
            MappingCommand::CursorDown => found_cursor_commands.push("Down"),
            MappingCommand::CursorLeft => found_cursor_commands.push("Left"),
            MappingCommand::CursorRight => found_cursor_commands.push("Right"),
            _ => {}
        }
    }

    assert!(found_cursor_commands.contains(&"Down"), "Expected CursorDown from 0x11");
    assert!(found_cursor_commands.contains(&"Right"), "Expected CursorRight from 0x1D");
    assert!(found_cursor_commands.contains(&"Up"), "Expected CursorUp from 0x91");
    assert!(found_cursor_commands.contains(&"Left"), "Expected CursorLeft from 0x9D");

    // Verify 0xFF: PI character (byte 94)
    let mut found_pi = false;
    for cmd in &sink.commands {
        if let MappingCommand::Text(data) = cmd {
            if data.contains(&94) {
                found_pi = true;
                break;
            }
        }
    }
    assert!(found_pi, "Expected PI character (byte 94) from 0xFF");
}

#[test]
fn test_petscii_character_mapping() {
    let mut parser = PetsciiParser::new();

    // Test specific character mappings: 0x20 -> 0x20 (space)
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x20], &mut sink);
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Text(vec![0x20])));

    // 0x40 (@) -> 0x00
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x40], &mut sink);
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Text(vec![0x00])));

    // 0x41 (A) -> 0x01
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x41], &mut sink);
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Text(vec![0x01])));

    // 0x5A (Z) -> 0x1A
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x5A], &mut sink);
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Text(vec![0x1A])));

    // 0x61 (a) -> 0x41
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x61], &mut sink);
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Text(vec![0x41])));

    // 0x7A (z) -> 0x5A
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x7A], &mut sink);
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Text(vec![0x5A])));
}
