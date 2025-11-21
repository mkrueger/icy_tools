use icy_parser_core::{CommandParser, CommandSink, ErrorLevel, ParseError, SkypixParser, TerminalCommand};

struct ErrorCapturingSink {
    errors: Vec<(ParseError, ErrorLevel)>,
    commands: Vec<TerminalCommand>,
}

impl ErrorCapturingSink {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            commands: Vec::new(),
        }
    }
}

impl CommandSink for ErrorCapturingSink {
    fn emit(&mut self, command: TerminalCommand) {
        self.commands.push(command);
    }

    fn report_error(&mut self, error: ParseError, level: ErrorLevel) {
        self.errors.push((error, level));
    }

    fn print(&mut self, _text: &[u8]) {
        // Not used in these tests
    }
}

#[test]
fn test_invalid_erase_display_mode() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Invalid mode 5 for Erase Display
    parser.parse(b"\x1b[5J", &mut sink);

    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0].0 {
        ParseError::InvalidParameter { command, value, expected } => {
            assert_eq!(*command, "EraseDisplay");
            assert_eq!(*value, "5");
            assert!(expected.as_ref().unwrap().contains("cursor to end"));
            assert!(expected.as_ref().unwrap().contains("start to cursor"));
            assert!(expected.as_ref().unwrap().contains("entire display"));
        }
        _ => panic!("Expected InvalidParameter error"),
    }
    assert_eq!(sink.errors[0].1, ErrorLevel::Warning);
}

#[test]
fn test_valid_erase_display_modes() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Valid modes: 0, 1, 2
    parser.parse(b"\x1b[0J\x1b[1J\x1b[2J", &mut sink);

    assert_eq!(sink.errors.len(), 0, "No errors should be reported for valid modes");
    assert_eq!(sink.commands.len(), 3, "Should emit 3 commands");
}

#[test]
fn test_invalid_erase_line_mode() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Invalid mode 3 for Erase Line
    parser.parse(b"\x1b[3K", &mut sink);

    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0].0 {
        ParseError::InvalidParameter { command, value, expected } => {
            assert_eq!(*command, "EraseLine");
            assert_eq!(*value, "3");
            assert!(expected.as_ref().unwrap().contains("cursor to end"));
            assert!(expected.as_ref().unwrap().contains("start to cursor"));
            assert!(expected.as_ref().unwrap().contains("entire line"));
        }
        _ => panic!("Expected InvalidParameter error"),
    }
    assert_eq!(sink.errors[0].1, ErrorLevel::Warning);
}

#[test]
fn test_valid_erase_line_modes() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Valid modes: 0, 1, 2
    parser.parse(b"\x1b[0K\x1b[1K\x1b[2K", &mut sink);

    assert_eq!(sink.errors.len(), 0, "No errors should be reported for valid modes");
    assert_eq!(sink.commands.len(), 3, "Should emit 3 commands");
}

#[test]
fn test_invalid_sgr_parameter() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Invalid SGR parameter 99
    parser.parse(b"\x1b[99m", &mut sink);

    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0].0 {
        ParseError::InvalidParameter { command, value, expected } => {
            assert_eq!(*command, "SGR");
            assert_eq!(*value, "99");
            assert!(expected.as_ref().unwrap().contains("0 (reset)"));
            assert!(expected.as_ref().unwrap().contains("30-37 (foreground)"));
            assert!(expected.as_ref().unwrap().contains("40-47 (background)"));
        }
        _ => panic!("Expected InvalidParameter error"),
    }
    assert_eq!(sink.errors[0].1, ErrorLevel::Warning);
}

#[test]
fn test_valid_sgr_parameters() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Valid SGR parameters: 0, 1, 3, 5, 7, 30-37, 40-47
    parser.parse(b"\x1b[0m\x1b[1m\x1b[3m\x1b[5m\x1b[7m\x1b[31m\x1b[42m", &mut sink);

    assert_eq!(sink.errors.len(), 0, "No errors should be reported for valid SGR parameters");
    assert!(sink.commands.len() > 0, "Should emit commands");
}

#[test]
fn test_multiple_sgr_parameters_with_invalid() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Mix of valid and invalid SGR parameters
    parser.parse(b"\x1b[1;99;31m", &mut sink);

    // Should emit commands for valid parameters and report error for invalid one
    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0].0 {
        ParseError::InvalidParameter { value, .. } => {
            assert_eq!(*value, "99");
        }
        _ => panic!("Expected InvalidParameter error"),
    }
    assert!(sink.commands.len() >= 2, "Should emit commands for valid parameters");
}

#[test]
fn test_invalid_character_in_csi_sequence() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Invalid character '#' in CSI parameter sequence
    parser.parse(b"\x1b[10#20m", &mut sink);

    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0].0 {
        ParseError::MalformedSequence {
            description,
            sequence,
            context,
        } => {
            assert!(description.contains("Invalid character"));
            assert!(sequence.is_some());
            assert!(context.as_ref().unwrap().contains("Expected digit"));
        }
        _ => panic!("Expected MalformedSequence error"),
    }
    assert_eq!(sink.errors[0].1, ErrorLevel::Warning);
}

#[test]
fn test_invalid_character_after_csi() {
    let mut parser = SkypixParser::new();
    let mut sink = ErrorCapturingSink::new();

    // Invalid character '@' immediately after CSI
    parser.parse(b"\x1b[@", &mut sink);

    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0].0 {
        ParseError::MalformedSequence { description, sequence, .. } => {
            assert!(description.contains("Invalid character after CSI"));
            assert!(sequence.as_ref().unwrap().contains("ESC[@"));
        }
        _ => panic!("Expected MalformedSequence error"),
    }
    assert_eq!(sink.errors[0].1, ErrorLevel::Warning);
}
