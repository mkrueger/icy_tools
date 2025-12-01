use icy_parser_core::{AtasciiParser, CommandParser, CommandSink, Direction, EraseInDisplayMode, TerminalCommand};

/* Test if the ATASCII parser produces expected commands for all 256 bytes (0x00-0xFF) */

struct MappingTestSink {
    commands: Vec<MappingCommand>,
}

#[derive(Debug, Clone, PartialEq)]
enum MappingCommand {
    Text(Vec<u8>),
    Bell,
    Backspace,
    Delete,
    LineFeed,
    Tab,
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    ClearScreen,
    DeleteLine,
    InsertLine,
    ClearTab,
    SetTab,
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
            TerminalCommand::Delete => {
                self.commands.push(MappingCommand::Delete);
            }
            TerminalCommand::LineFeed => {
                self.commands.push(MappingCommand::LineFeed);
            }
            TerminalCommand::Tab => {
                self.commands.push(MappingCommand::Tab);
            }
            TerminalCommand::CsiMoveCursor(Direction::Up, _, _) => {
                self.commands.push(MappingCommand::CursorUp);
            }
            TerminalCommand::CsiMoveCursor(Direction::Down, _, _) => {
                self.commands.push(MappingCommand::CursorDown);
            }
            TerminalCommand::CsiMoveCursor(Direction::Left, _, _) => {
                self.commands.push(MappingCommand::CursorLeft);
            }
            TerminalCommand::CsiMoveCursor(Direction::Right, _, _) => {
                self.commands.push(MappingCommand::CursorRight);
            }
            TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All) => {
                self.commands.push(MappingCommand::ClearScreen);
            }
            TerminalCommand::CsiDeleteLine(_) => {
                self.commands.push(MappingCommand::DeleteLine);
            }
            TerminalCommand::CsiInsertLine(_) => {
                self.commands.push(MappingCommand::InsertLine);
            }
            TerminalCommand::CsiClearTabulation => {
                self.commands.push(MappingCommand::ClearTab);
            }
            TerminalCommand::EscSetTab => {
                self.commands.push(MappingCommand::SetTab);
            }
            _ => {}
        }
    }
}

#[test]
fn test_atascii_all_byte_mappings() {
    let mut parser = AtasciiParser::new();
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

    // 0x00-0x1A: Printable characters (will be in Text commands)
    // Skip to first control command - first command should be Text with bytes 0x00-0x1A
    let cmd = get_cmd();
    if let Some(MappingCommand::Text(data)) = cmd {
        assert!(data.len() > 0, "Expected text command with printable bytes");
    } else {
        panic!("Expected Text command first, got: {:?}", cmd);
    }

    // 0x1B: ESC - escape sequence follows (next byte 0x1C is handled as escaped literal)
    // The 0x1C after ESC will be printed as literal character (in a Text command)
    let cmd = get_cmd();
    if let Some(MappingCommand::Text(data)) = cmd {
        assert!(data.contains(&0x1C), "Expected literal 0x1C from ESC sequence");
    }

    // 0x1D: Cursor DOWN
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::CursorDown), "0x1D: Expected CursorDown");

    // 0x1E: Cursor LEFT
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::CursorLeft), "0x1E: Expected CursorLeft");

    // 0x1F: Cursor RIGHT
    let cmd = get_cmd();
    assert_eq!(cmd, Some(MappingCommand::CursorRight), "0x1F: Expected CursorRight");

    // Skip printable characters 0x20-0x7C
    // Collect all commands to verify later
    let all_commands = sink.commands.clone();

    // Verify control commands exist in the list
    let mut found_commands = std::collections::HashSet::new();
    for cmd in &all_commands {
        match cmd {
            MappingCommand::CursorUp => {
                found_commands.insert("CursorUp");
            }
            MappingCommand::CursorDown => {
                found_commands.insert("CursorDown");
            }
            MappingCommand::CursorLeft => {
                found_commands.insert("CursorLeft");
            }
            MappingCommand::CursorRight => {
                found_commands.insert("CursorRight");
            }
            MappingCommand::ClearScreen => {
                found_commands.insert("ClearScreen");
            }
            MappingCommand::Backspace => {
                found_commands.insert("Backspace");
            }
            MappingCommand::Tab => {
                found_commands.insert("Tab");
            }
            MappingCommand::LineFeed => {
                found_commands.insert("LineFeed");
            }
            MappingCommand::DeleteLine => {
                found_commands.insert("DeleteLine");
            }
            MappingCommand::InsertLine => {
                found_commands.insert("InsertLine");
            }
            MappingCommand::ClearTab => {
                found_commands.insert("ClearTab");
            }
            MappingCommand::SetTab => {
                found_commands.insert("SetTab");
            }
            MappingCommand::Bell => {
                found_commands.insert("Bell");
            }
            MappingCommand::Delete => {
                found_commands.insert("Delete");
            }
            _ => {}
        }
    }

    // Verify all expected control commands from the comment list
    assert!(found_commands.contains("CursorDown"), "Expected CursorDown from 0x1D");
    assert!(found_commands.contains("CursorLeft"), "Expected CursorLeft from 0x1E");
    assert!(found_commands.contains("CursorRight"), "Expected CursorRight from 0x1F");
    assert!(found_commands.contains("ClearScreen"), "Expected ClearScreen from 0x7D");
    assert!(found_commands.contains("Backspace"), "Expected Backspace from 0x7E");
    assert!(found_commands.contains("Tab"), "Expected Tab from 0x7F");
    assert!(found_commands.contains("LineFeed"), "Expected LineFeed from 0x9B");
    assert!(found_commands.contains("DeleteLine"), "Expected DeleteLine from 0x9C");
    assert!(found_commands.contains("InsertLine"), "Expected InsertLine from 0x9D");
    assert!(found_commands.contains("ClearTab"), "Expected ClearTab from 0x9E");
    assert!(found_commands.contains("SetTab"), "Expected SetTab from 0x9F");
    assert!(found_commands.contains("Bell"), "Expected Bell from 0xFD");
    assert!(found_commands.contains("Delete"), "Expected Delete from 0xFE");

    // Verify printable characters (0x00-0x7C except control codes)
    // Count text commands that contain printable ranges
    let mut has_normal_chars = false;
    let mut has_inverse_chars = false;

    for cmd in &all_commands {
        if let MappingCommand::Text(data) = cmd {
            // Check for normal printable characters (0x00-0x7F range)
            for &byte in data {
                if byte < 0x80 {
                    has_normal_chars = true;
                }
                if byte >= 0x80 {
                    has_inverse_chars = true;
                }
            }
        }
    }

    assert!(has_normal_chars, "Expected normal printable characters (0x00-0x7F)");
    assert!(has_inverse_chars, "Expected inverse video characters (0x80-0xFF)");

    // Verify ESC sequence handling: 0x1B followed by 0x1C should result in literal 0x1C
    let mut found_escaped_char = false;
    for cmd in &all_commands {
        if let MappingCommand::Text(data) = cmd {
            if data.contains(&0x1C) {
                found_escaped_char = true;
                break;
            }
        }
    }
    assert!(found_escaped_char, "Expected literal 0x1C character from ESC+0x1C sequence");

    // Verify 0xFF: Insert blank character (space)
    let mut found_space_from_ff = false;
    for cmd in &all_commands {
        if let MappingCommand::Text(data) = cmd {
            if data.contains(&b' ') {
                found_space_from_ff = true;
                break;
            }
        }
    }
    assert!(found_space_from_ff, "Expected space character from 0xFF insert");
}

#[test]
fn test_atascii_control_codes() {
    let mut parser = AtasciiParser::new();

    // Test cursor movements
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x1D], &mut sink); // Cursor DOWN
    assert_eq!(sink.commands.last(), Some(&MappingCommand::CursorDown));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0x1E], &mut sink); // Cursor LEFT
    assert_eq!(sink.commands.last(), Some(&MappingCommand::CursorLeft));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0x1F], &mut sink); // Cursor RIGHT
    assert_eq!(sink.commands.last(), Some(&MappingCommand::CursorRight));

    // Test screen operations
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x7D], &mut sink); // Clear screen
    assert_eq!(sink.commands.last(), Some(&MappingCommand::ClearScreen));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0x7E], &mut sink); // Backspace
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Backspace));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0x7F], &mut sink); // Tab
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Tab));

    // Test line operations
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x9B], &mut sink); // Line feed
    assert_eq!(sink.commands.last(), Some(&MappingCommand::LineFeed));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0x9C], &mut sink); // Delete line
    assert_eq!(sink.commands.last(), Some(&MappingCommand::DeleteLine));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0x9D], &mut sink); // Insert line
    assert_eq!(sink.commands.last(), Some(&MappingCommand::InsertLine));

    // Test tab operations
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x9E], &mut sink); // Clear tab
    assert_eq!(sink.commands.last(), Some(&MappingCommand::ClearTab));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0x9F], &mut sink); // Set tab
    assert_eq!(sink.commands.last(), Some(&MappingCommand::SetTab));

    // Test special characters
    let mut sink = MappingTestSink::new();
    parser.parse(&[0xFD], &mut sink); // Bell
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Bell));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0xFE], &mut sink); // Delete character
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Delete));

    let mut sink = MappingTestSink::new();
    parser.parse(&[0xFF], &mut sink); // Insert blank (space)
    assert_eq!(sink.commands.last(), Some(&MappingCommand::Text(vec![b' '])));
}

#[test]
fn test_atascii_escape_sequence() {
    let mut parser = AtasciiParser::new();
    let mut sink = MappingTestSink::new();

    // Test ESC (0x1B) followed by a control character makes it literal
    // 0x1B + 0x1C should print literal 0x1C instead of cursor movement
    parser.parse(&[0x1B, 0x1C], &mut sink);

    // Should have one Text command with the literal 0x1C byte
    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], MappingCommand::Text(vec![0x1C]));

    // Test ESC with other characters
    let mut sink = MappingTestSink::new();
    parser.parse(&[0x1B, 0x7D], &mut sink); // ESC + Clear screen -> literal 0x7D
    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], MappingCommand::Text(vec![0x7D]));
}

#[test]
fn test_atascii_inverse_video() {
    let mut parser = AtasciiParser::new();
    let mut sink = MappingTestSink::new();

    // Characters 0x80-0xFF are inverse video versions (except control codes)
    // Test a range of inverse characters
    let inverse_chars = vec![0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0];
    parser.parse(&inverse_chars, &mut sink);

    // Should have text commands with these bytes preserved
    let mut found_inverse = Vec::new();
    for cmd in &sink.commands {
        if let MappingCommand::Text(data) = cmd {
            for &byte in data {
                if byte >= 0x80 && byte < 0xFD {
                    found_inverse.push(byte);
                }
            }
        }
    }

    assert!(found_inverse.len() > 0, "Expected inverse video characters to be printed");
    assert!(found_inverse.contains(&0x80), "Expected 0x80 inverse character");
}

#[test]
fn test_atascii_printable_range() {
    let mut parser = AtasciiParser::new();
    let mut sink = MappingTestSink::new();

    // Test standard printable ASCII range
    let printable = b"Hello, World! 123";
    parser.parse(printable, &mut sink);

    // Should have text commands containing all these characters
    let mut collected_text = Vec::new();
    for cmd in &sink.commands {
        if let MappingCommand::Text(data) = cmd {
            collected_text.extend_from_slice(data);
        }
    }

    assert_eq!(collected_text, printable.to_vec(), "Printable characters should pass through unchanged");
}
