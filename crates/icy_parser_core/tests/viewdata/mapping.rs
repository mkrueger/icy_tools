use icy_parser_core::{
    Blink, Color, CommandParser, CommandSink, Direction, EraseInDisplayMode, SgrAttribute, TerminalCommand, ViewDataCommand, ViewdataParser,
};

#[derive(Debug, PartialEq)]
enum MappingCommand {
    Text(Vec<u8>),
    CursorLeft,
    CursorRight,
    CursorDown,
    CursorUp,
    ClearScreen,
    CarriageReturn,
    Home,
    SetForeground(Color),
    SetBackground(Color),
    Blink(bool),
    Concealed(bool),
}

struct MappingTestSink {
    commands: Vec<MappingCommand>,
}

impl MappingTestSink {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }

    fn get_cmd(&mut self, index: usize) -> Option<MappingCommand> {
        if index < self.commands.len() {
            Some(self.commands.remove(index))
        } else {
            None
        }
    }
}

impl CommandSink for MappingTestSink {
    fn print(&mut self, text: &[u8]) {
        self.commands.push(MappingCommand::Text(text.to_vec()));
    }

    fn emit(&mut self, command: TerminalCommand) {
        match command {
            TerminalCommand::CsiMoveCursor(Direction::Left, _) => {
                self.commands.push(MappingCommand::CursorLeft);
            }
            TerminalCommand::CsiMoveCursor(Direction::Right, _) => {
                self.commands.push(MappingCommand::CursorRight);
            }
            TerminalCommand::CsiMoveCursor(Direction::Down, _) => {
                self.commands.push(MappingCommand::CursorDown);
            }
            TerminalCommand::CsiMoveCursor(Direction::Up, _) => {
                self.commands.push(MappingCommand::CursorUp);
            }
            TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All) => {
                self.commands.push(MappingCommand::ClearScreen);
            }
            TerminalCommand::CarriageReturn => {
                self.commands.push(MappingCommand::CarriageReturn);
            }
            TerminalCommand::CsiCursorPosition(1, 1) => {
                self.commands.push(MappingCommand::Home);
            }
            TerminalCommand::CsiSelectGraphicRendition(attr) => match attr {
                SgrAttribute::Foreground(color) => {
                    self.commands.push(MappingCommand::SetForeground(color));
                }
                SgrAttribute::Background(color) => {
                    self.commands.push(MappingCommand::SetBackground(color));
                }
                SgrAttribute::Blink(blink) => {
                    self.commands.push(MappingCommand::Blink(blink != Blink::Off));
                }
                SgrAttribute::Concealed(concealed) => {
                    self.commands.push(MappingCommand::Concealed(concealed));
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn emit_view_data(&mut self, cmd: ViewDataCommand) -> bool {
        match cmd {
            ViewDataCommand::ViewDataClearScreen => {
                self.commands.push(MappingCommand::ClearScreen);
                self.commands.push(MappingCommand::Home);
            }
            ViewDataCommand::SetChar(ch) => {
                self.commands.push(MappingCommand::Text(vec![ch]));
            }
            ViewDataCommand::MoveCaret(Direction::Up) => {
                self.commands.push(MappingCommand::CursorUp);
            }
            ViewDataCommand::MoveCaret(Direction::Down) => {
                self.commands.push(MappingCommand::CursorDown);
            }
            ViewDataCommand::MoveCaret(Direction::Left) => {
                self.commands.push(MappingCommand::CursorLeft);
            }
            ViewDataCommand::MoveCaret(Direction::Right) => {
                self.commands.push(MappingCommand::CursorRight);
            }
            _ => {}
        }
        false
    }
}

#[test]
fn test_viewdata_all_byte_mappings() {
    let mut parser = ViewdataParser::default();
    let mut sink = MappingTestSink::new();

    // Send all 256 bytes
    let mut data = Vec::new();
    for i in 0..=255 {
        data.push(i);
    }

    parser.parse(&data, &mut sink);

    // Check we got commands (there should be many)
    assert!(!sink.commands.is_empty(), "No commands generated from 256 bytes");

    // VIEWDATA parser: bytes 0x00-0x07 are control codes that don't generate output
    // The first actual commands are from cursor movements starting at 0x08

    // Verify cursor movement commands (0x08-0x0B)
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorLeft)); // 0x08
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorRight)); // 0x09
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorDown)); // 0x0A
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorUp)); // 0x0B

    // Clear screen (0x0C) - emits ClearScreen and Home
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::ClearScreen));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Home));

    // Carriage return (0x0D)
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CarriageReturn));

    // Home (0x1E)
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Home));

    // Now we should see text commands for printable characters starting at 0x20 (space)
    // Verify we have text commands
    let mut text_count = 0;
    while !sink.commands.is_empty() {
        if let Some(MappingCommand::Text(_)) = sink.get_cmd(0) {
            text_count += 1;
        }
    }

    assert!(text_count > 0, "Expected some text commands for printable characters");
}

#[test]
fn test_viewdata_esc_alpha_colors() {
    let mut sink = MappingTestSink::new();

    // Test ESC + A-G for alpha colors (Red, Green, Yellow, Blue, Magenta, Cyan, White)
    let colors = [
        (b'A', 1), // Red
        (b'B', 2), // Green
        (b'C', 3), // Yellow
        (b'D', 4), // Blue
        (b'E', 5), // Magenta
        (b'F', 6), // Cyan
        (b'G', 7), // White
    ];

    for (letter, color_value) in colors.iter() {
        sink.commands.clear();
        let mut parser = ViewdataParser::default();

        parser.parse(&[0x1B, *letter], &mut sink);

        // Parser first emits space character, then Concealed and SetForeground
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::Text(vec![b' '])));
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorRight));
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::Concealed(false)));
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::SetForeground(Color::Base(*color_value))));
    }
}

#[test]
fn test_viewdata_esc_graphic_colors() {
    let mut sink = MappingTestSink::new();

    // Test ESC + Q-W for graphics colors (Red, Green, Yellow, Blue, Magenta, Cyan, White)
    let colors = [
        (b'Q', 1), // Red
        (b'R', 2), // Green
        (b'S', 3), // Yellow
        (b'T', 4), // Blue
        (b'U', 5), // Magenta
        (b'V', 6), // Cyan
        (b'W', 7), // White
    ];

    for (letter, color_value) in colors.iter() {
        sink.commands.clear();
        let mut parser = ViewdataParser::default();

        parser.parse(&[0x1B, *letter], &mut sink);

        // Parser first emits space character, then Concealed and SetForeground
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::Text(vec![b' '])));
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorRight));
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::Concealed(false)));
        assert_eq!(sink.get_cmd(0), Some(MappingCommand::SetForeground(Color::Base(*color_value))));
    }
}

#[test]
fn test_viewdata_esc_flash_steady() {
    let mut sink = MappingTestSink::new();

    // Test ESC + H (Flash on)
    let mut parser = ViewdataParser::default();
    parser.parse(&[0x1B, b'H'], &mut sink);
    // Parser first emits space character, then Blink (ESC+H is processed after SetChar)
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Text(vec![b' '])));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorRight));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Blink(true)));

    // Test ESC + I (Steady - blink off)
    sink.commands.clear();
    parser = ViewdataParser::default();
    parser.parse(&[0x1B, b'I'], &mut sink);
    // For ESC+I: Blink command comes first, then space character
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Blink(false)));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Text(vec![b' '])));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorRight));
}

#[test]
fn test_viewdata_esc_background() {
    let mut parser = ViewdataParser::default();
    let mut sink = MappingTestSink::new();

    // Test ESC + \ (Black background)
    parser.parse(&[0x1B, b'\\'], &mut sink);
    // For ESC+\: Concealed and SetBackground come first, then space character
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Concealed(false)));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::SetBackground(Color::Base(0))));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Text(vec![b' '])));
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::CursorRight));
}

#[test]
fn test_viewdata_esc_conceal() {
    let mut parser = ViewdataParser::default();
    let mut sink = MappingTestSink::new();

    // Test ESC + X (Conceal in alpha mode)
    parser.parse(&[0x1B, b'X'], &mut sink);
    assert_eq!(sink.get_cmd(0), Some(MappingCommand::Concealed(true)));
}

/*
[VIEWDATA] Input byte: 0x00 (NUL)
[VIEWDATA] 0x00: NUL (ignored)
[VIEWDATA] Input byte: 0x01 (.)
[VIEWDATA] 0x01: SOH (ignored)
[VIEWDATA] Input byte: 0x02 (.)
[VIEWDATA] 0x02: STX
[VIEWDATA] Input byte: 0x03 (.)
[VIEWDATA] 0x03: ETX
[VIEWDATA] Input byte: 0x04 (.)
[VIEWDATA] 0x04: EOT (ignored)
[VIEWDATA] Input byte: 0x05 (.)
[VIEWDATA] 0x05: ENQ (send identity)
[VIEWDATA] Input byte: 0x06 (.)
[VIEWDATA] 0x06: ACK
[VIEWDATA] Input byte: 0x07 (.)
[VIEWDATA] 0x07: BEL (ignored)
[VIEWDATA] Input byte: 0x08 (.)
[VIEWDATA] 0x08: Cursor LEFT
[VIEWDATA] Input byte: 0x09 (.)
[VIEWDATA] 0x09: Cursor RIGHT
[VIEWDATA] Input byte: 0x0A (.)
[VIEWDATA] 0x0A: Cursor DOWN + reset row
[VIEWDATA] Input byte: 0x0B (.)
[VIEWDATA] 0x0B: Cursor UP
[VIEWDATA] Input byte: 0x0C (.)
[VIEWDATA] 0x0C: Form feed/Clear screen + reset
[VIEWDATA] Input byte: 0x0D (.)
[VIEWDATA] 0x0D: Carriage return
[VIEWDATA] Input byte: 0x0E (.)
[VIEWDATA] 0x0E: SO - switch to G1 charset (TODO)
[VIEWDATA] Input byte: 0x0F (.)
[VIEWDATA] 0x0F: SI - switch to G0 charset (TODO)
[VIEWDATA] Input byte: 0x10 (.)
[VIEWDATA] 0x10: DLE (ignored)
[VIEWDATA] Input byte: 0x11 (.)
[VIEWDATA] 0x11: DC1 - Show cursor
[VIEWDATA] Input byte: 0x12 (.)
[VIEWDATA] 0x12: DC2 (ignored)
[VIEWDATA] Input byte: 0x13 (.)
[VIEWDATA] 0x13: DC3 (ignored)
[VIEWDATA] Input byte: 0x14 (.)
[VIEWDATA] 0x14: DC4 - Hide cursor
[VIEWDATA] Input byte: 0x15 (.)
[VIEWDATA] 0x15: NAK
[VIEWDATA] Input byte: 0x16 (.)
[VIEWDATA] 0x16: SYN (ignored)
[VIEWDATA] Input byte: 0x17 (.)
[VIEWDATA] 0x17: ETB (ignored)
[VIEWDATA] Input byte: 0x18 (.)
[VIEWDATA] 0x18: CAN
[VIEWDATA] Input byte: 0x19 (.)
[VIEWDATA] 0x19: EM (ignored)
[VIEWDATA] Input byte: 0x1A (.)
[VIEWDATA] 0x1A: SUB (ignored)
[VIEWDATA] Input byte: 0x1B (.)
[VIEWDATA] 0x1B: ESC - escape sequence follows
[VIEWDATA] Input byte: 0x1C (.)
[VIEWDATA] 0x1C: SS2 - switch to G2 charset (TODO)
[VIEWDATA] Input byte: 0x1D (.)
[VIEWDATA] 0x1D: SS3 - switch to G3 charset (TODO)
[VIEWDATA] Input byte: 0x1E (.)
[VIEWDATA] 0x1E: Home cursor
[VIEWDATA] Input byte: 0x1F (.)
[VIEWDATA] 0x1F: US (ignored)
[VIEWDATA] Input byte: 0x20 (.)
[VIEWDATA] interpret_char: 0x20, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x20 ( )
[VIEWDATA] Input byte: 0x21 (!)
[VIEWDATA] interpret_char: 0x21, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x21 (!)
[VIEWDATA] Input byte: 0x22 (")
[VIEWDATA] interpret_char: 0x22, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x22 (")
[VIEWDATA] Input byte: 0x23 (#)
[VIEWDATA] interpret_char: 0x23, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x23 (#)
[VIEWDATA] Input byte: 0x24 ($)
[VIEWDATA] interpret_char: 0x24, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x24 ($)
[VIEWDATA] Input byte: 0x25 (%)
[VIEWDATA] interpret_char: 0x25, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x25 (%)
[VIEWDATA] Input byte: 0x26 (&)
[VIEWDATA] interpret_char: 0x26, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x26 (&)
[VIEWDATA] Input byte: 0x27 (')
[VIEWDATA] interpret_char: 0x27, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x27 (')
[VIEWDATA] Input byte: 0x28 (()
[VIEWDATA] interpret_char: 0x28, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x28 (()
[VIEWDATA] Input byte: 0x29 ())
[VIEWDATA] interpret_char: 0x29, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x29 ())
[VIEWDATA] Input byte: 0x2A (*)
[VIEWDATA] interpret_char: 0x2A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x2A (*)
[VIEWDATA] Input byte: 0x2B (+)
[VIEWDATA] interpret_char: 0x2B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x2B (+)
[VIEWDATA] Input byte: 0x2C (,)
[VIEWDATA] interpret_char: 0x2C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x2C (,)
[VIEWDATA] Input byte: 0x2D (-)
[VIEWDATA] interpret_char: 0x2D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x2D (-)
[VIEWDATA] Input byte: 0x2E (.)
[VIEWDATA] interpret_char: 0x2E, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x2E (.)
[VIEWDATA] Input byte: 0x2F (/)
[VIEWDATA] interpret_char: 0x2F, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x2F (/)
[VIEWDATA] Input byte: 0x30 (0)
[VIEWDATA] interpret_char: 0x30, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x30 (0)
[VIEWDATA] Input byte: 0x31 (1)
[VIEWDATA] interpret_char: 0x31, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x31 (1)
[VIEWDATA] Input byte: 0x32 (2)
[VIEWDATA] interpret_char: 0x32, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x32 (2)
[VIEWDATA] Input byte: 0x33 (3)
[VIEWDATA] interpret_char: 0x33, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x33 (3)
[VIEWDATA] Input byte: 0x34 (4)
[VIEWDATA] interpret_char: 0x34, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x34 (4)
[VIEWDATA] Input byte: 0x35 (5)
[VIEWDATA] interpret_char: 0x35, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x35 (5)
[VIEWDATA] Input byte: 0x36 (6)
[VIEWDATA] interpret_char: 0x36, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x36 (6)
[VIEWDATA] Input byte: 0x37 (7)
[VIEWDATA] interpret_char: 0x37, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x37 (7)
[VIEWDATA] Input byte: 0x38 (8)
[VIEWDATA] interpret_char: 0x38, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x38 (8)
[VIEWDATA] Input byte: 0x39 (9)
[VIEWDATA] interpret_char: 0x39, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x39 (9)
[VIEWDATA] Input byte: 0x3A (:)
[VIEWDATA] interpret_char: 0x3A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x3A (:)
[VIEWDATA] Input byte: 0x3B (;)
[VIEWDATA] interpret_char: 0x3B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x3B (;)
[VIEWDATA] Input byte: 0x3C (<)
[VIEWDATA] interpret_char: 0x3C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x3C (<)
[VIEWDATA] Input byte: 0x3D (=)
[VIEWDATA] interpret_char: 0x3D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x3D (=)
[VIEWDATA] Input byte: 0x3E (>)
[VIEWDATA] interpret_char: 0x3E, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x3E (>)
[VIEWDATA] Input byte: 0x3F (?)
[VIEWDATA] interpret_char: 0x3F, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x3F (?)
[VIEWDATA] Input byte: 0x40 (@)
[VIEWDATA] interpret_char: 0x40, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x40 (@)
[VIEWDATA] Input byte: 0x41 (A)
[VIEWDATA] interpret_char: 0x41, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x41 (A)
[VIEWDATA] Input byte: 0x42 (B)
[VIEWDATA] interpret_char: 0x42, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x42 (B)
[VIEWDATA] Input byte: 0x43 (C)
[VIEWDATA] interpret_char: 0x43, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x43 (C)
[VIEWDATA] Input byte: 0x44 (D)
[VIEWDATA] interpret_char: 0x44, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x44 (D)
[VIEWDATA] Input byte: 0x45 (E)
[VIEWDATA] interpret_char: 0x45, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x45 (E)
[VIEWDATA] Input byte: 0x46 (F)
[VIEWDATA] interpret_char: 0x46, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x46 (F)
[VIEWDATA] Input byte: 0x47 (G)
[VIEWDATA] interpret_char: 0x47, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x47 (G)
[VIEWDATA] Input byte: 0x48 (H)
[VIEWDATA] interpret_char: 0x48, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x48 (H)
[VIEWDATA] Input byte: 0x49 (I)
[VIEWDATA] interpret_char: 0x49, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x49 (I)
[VIEWDATA] Input byte: 0x4A (J)
[VIEWDATA] interpret_char: 0x4A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x4A (J)
[VIEWDATA] Input byte: 0x4B (K)
[VIEWDATA] interpret_char: 0x4B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x4B (K)
[VIEWDATA] Input byte: 0x4C (L)
[VIEWDATA] interpret_char: 0x4C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x4C (L)
[VIEWDATA] Input byte: 0x4D (M)
[VIEWDATA] interpret_char: 0x4D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x4D (M)
[VIEWDATA] Input byte: 0x4E (N)
[VIEWDATA] interpret_char: 0x4E, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x4E (N)
[VIEWDATA] Input byte: 0x4F (O)
[VIEWDATA] interpret_char: 0x4F, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x4F (O)
[VIEWDATA] Input byte: 0x50 (P)
[VIEWDATA] interpret_char: 0x50, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x50 (P)
[VIEWDATA] Input byte: 0x51 (Q)
[VIEWDATA] interpret_char: 0x51, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x51 (Q)
[VIEWDATA] Input byte: 0x52 (R)
[VIEWDATA] interpret_char: 0x52, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x52 (R)
[VIEWDATA] Input byte: 0x53 (S)
[VIEWDATA] interpret_char: 0x53, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x53 (S)
[VIEWDATA] Input byte: 0x54 (T)
[VIEWDATA] interpret_char: 0x54, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x54 (T)
[VIEWDATA] Input byte: 0x55 (U)
[VIEWDATA] interpret_char: 0x55, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x55 (U)
[VIEWDATA] Input byte: 0x56 (V)
[VIEWDATA] interpret_char: 0x56, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x56 (V)
[VIEWDATA] Input byte: 0x57 (W)
[VIEWDATA] interpret_char: 0x57, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x57 (W)
[VIEWDATA] Input byte: 0x58 (X)
[VIEWDATA] interpret_char: 0x58, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x58 (X)
[VIEWDATA] Input byte: 0x59 (Y)
[VIEWDATA] interpret_char: 0x59, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x59 (Y)
[VIEWDATA] Input byte: 0x5A (Z)
[VIEWDATA] interpret_char: 0x5A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x5A (Z)
[VIEWDATA] Input byte: 0x5B ([)
[VIEWDATA] interpret_char: 0x5B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x5B ([)
[VIEWDATA] Input byte: 0x5C (\)
[VIEWDATA] interpret_char: 0x5C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x5C (\)
[VIEWDATA] Input byte: 0x5D (])
[VIEWDATA] interpret_char: 0x5D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x5D (])
[VIEWDATA] Input byte: 0x5E (^)
[VIEWDATA] interpret_char: 0x5E, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x5E (^)
[VIEWDATA] Input byte: 0x5F (_)
[VIEWDATA] interpret_char: 0x5F, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x5F (_)
[VIEWDATA] Input byte: 0x60 (`)
[VIEWDATA] interpret_char: 0x60, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x60 (`)
[VIEWDATA] Input byte: 0x61 (a)
[VIEWDATA] interpret_char: 0x61, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x61 (a)
[VIEWDATA] Input byte: 0x62 (b)
[VIEWDATA] interpret_char: 0x62, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x62 (b)
[VIEWDATA] Input byte: 0x63 (c)
[VIEWDATA] interpret_char: 0x63, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x63 (c)
[VIEWDATA] Input byte: 0x64 (d)
[VIEWDATA] interpret_char: 0x64, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x64 (d)
[VIEWDATA] Input byte: 0x65 (e)
[VIEWDATA] interpret_char: 0x65, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x65 (e)
[VIEWDATA] Input byte: 0x66 (f)
[VIEWDATA] interpret_char: 0x66, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x66 (f)
[VIEWDATA] Input byte: 0x67 (g)
[VIEWDATA] interpret_char: 0x67, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x67 (g)
[VIEWDATA] Input byte: 0x68 (h)
[VIEWDATA] interpret_char: 0x68, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x68 (h)
[VIEWDATA] Input byte: 0x69 (i)
[VIEWDATA] interpret_char: 0x69, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x69 (i)
[VIEWDATA] Input byte: 0x6A (j)
[VIEWDATA] interpret_char: 0x6A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x6A (j)
[VIEWDATA] Input byte: 0x6B (k)
[VIEWDATA] interpret_char: 0x6B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x6B (k)
[VIEWDATA] Input byte: 0x6C (l)
[VIEWDATA] interpret_char: 0x6C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x6C (l)
[VIEWDATA] Input byte: 0x6D (m)
[VIEWDATA] interpret_char: 0x6D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x6D (m)
[VIEWDATA] Input byte: 0x6E (n)
[VIEWDATA] interpret_char: 0x6E, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x6E (n)
[VIEWDATA] Input byte: 0x6F (o)
[VIEWDATA] interpret_char: 0x6F, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x6F (o)
[VIEWDATA] Input byte: 0x70 (p)
[VIEWDATA] interpret_char: 0x70, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x70 (p)
[VIEWDATA] Input byte: 0x71 (q)
[VIEWDATA] interpret_char: 0x71, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x71 (q)
[VIEWDATA] Input byte: 0x72 (r)
[VIEWDATA] interpret_char: 0x72, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x72 (r)
[VIEWDATA] Input byte: 0x73 (s)
[VIEWDATA] interpret_char: 0x73, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x73 (s)
[VIEWDATA] Input byte: 0x74 (t)
[VIEWDATA] interpret_char: 0x74, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x74 (t)
[VIEWDATA] Input byte: 0x75 (u)
[VIEWDATA] interpret_char: 0x75, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x75 (u)
[VIEWDATA] Input byte: 0x76 (v)
[VIEWDATA] interpret_char: 0x76, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x76 (v)
[VIEWDATA] Input byte: 0x77 (w)
[VIEWDATA] interpret_char: 0x77, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x77 (w)
[VIEWDATA] Input byte: 0x78 (x)
[VIEWDATA] interpret_char: 0x78, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x78 (x)
[VIEWDATA] Input byte: 0x79 (y)
[VIEWDATA] interpret_char: 0x79, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x79 (y)
[VIEWDATA] Input byte: 0x7A (z)
[VIEWDATA] interpret_char: 0x7A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x7A (z)
[VIEWDATA] Input byte: 0x7B ({)
[VIEWDATA] interpret_char: 0x7B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x7B ({)
[VIEWDATA] Input byte: 0x7C (|)
[VIEWDATA] interpret_char: 0x7C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x7C (|)
[VIEWDATA] Input byte: 0x7D (})
[VIEWDATA] interpret_char: 0x7D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x7D (})
[VIEWDATA] Input byte: 0x7E (~)
[VIEWDATA] interpret_char: 0x7E, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x7E (~)
[VIEWDATA] Input byte: 0x7F (.)
[VIEWDATA] interpret_char: 0x7F, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x7F ()
[VIEWDATA] Input byte: 0x80 (.)
[VIEWDATA] interpret_char: 0x80, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x80 ()
[VIEWDATA] Input byte: 0x81 (.)
[VIEWDATA] interpret_char: 0x81, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x81 ()
[VIEWDATA] Input byte: 0x82 (.)
[VIEWDATA] interpret_char: 0x82, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x82 ()
[VIEWDATA] Input byte: 0x83 (.)
[VIEWDATA] interpret_char: 0x83, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x83 ()
[VIEWDATA] Input byte: 0x84 (.)
[VIEWDATA] interpret_char: 0x84, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x84 (
                               )
[VIEWDATA] Input byte: 0x85 (.)
[VIEWDATA] interpret_char: 0x85, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x85 (
)
[VIEWDATA] Input byte: 0x86 (.)
[VIEWDATA] interpret_char: 0x86, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x86 ()
[VIEWDATA] Input byte: 0x87 (.)
[VIEWDATA] interpret_char: 0x87, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x87 ()
[VIEWDATA] Input byte: 0x88 (.)
[VIEWDATA] interpret_char: 0x88, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x88 ()
[VIEWDATA] Input byte: 0x89 (.)
[VIEWDATA] interpret_char: 0x89, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x89 ()
[VIEWDATA] Input byte: 0x8A (.)
[VIEWDATA] interpret_char: 0x8A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x8A ()
[VIEWDATA] Input byte: 0x8B (.)
[VIEWDATA] interpret_char: 0x8B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x8B ()
[VIEWDATA] Input byte: 0x8C (.)
[VIEWDATA] interpret_char: 0x8C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x8C ()
[VIEWDATA] Input byte: 0x8D (.)
[VIEWDATA] interpret_char: 0x8D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x8D ()
[VIEWDATA] Input byte: 0x8E (.)
[VIEWDATA] interpret_char: 0x8E, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x8E ()
[VIEWDATA] Input byte: 0x8F (.)
[VIEWDATA] interpret_char: 0x8F, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x8F ()
[VIEWDATA] Input byte: 0x90 (.)
[VIEWDATA] interpret_char: 0x90, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x90 ()
[VIEWDATA] Input byte: 0x92 (.)
[VIEWDATA] interpret_char: 0x92, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x92 ()
[VIEWDATA] Input byte: 0x93 (.)
[VIEWDATA] interpret_char: 0x93, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x93 ()
[VIEWDATA] Input byte: 0x94 (.)
[VIEWDATA] interpret_char: 0x94, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x94 ()
[VIEWDATA] Input byte: 0x95 (.)
[VIEWDATA] interpret_char: 0x95, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x95 ()
[VIEWDATA] Input byte: 0x96 (.)
[VIEWDATA] interpret_char: 0x96, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x96 ()
[VIEWDATA] Input byte: 0x97 (.)
[VIEWDATA] interpret_char: 0x97, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x97 ()
[VIEWDATA] Input byte: 0x98 (.)
[VIEWDATA] interpret_char: 0x98, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x98 ()
[VIEWDATA] Input byte: 0x9A (.)
[VIEWDATA] interpret_char: 0x9A, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x9A ()
[VIEWDATA] Input byte: 0x9B (.)
[VIEWDATA] interpret_char: 0x9B, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x9B (
VIEWDATA] Input byte: 0x9C (.)
[VIEWDATA] interpret_char: 0x9C, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x9C ()
[VIEWDATA] Input byte: 0x9D (.)
[VIEWDATA] interpret_char: 0x9D, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0x9D ()
[VIEWDATA] Input byte: 0xA1 (.)
[VIEWDATA] interpret_char: 0xA1, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA1 (¡)
[VIEWDATA] Input byte: 0xA2 (.)
[VIEWDATA] interpret_char: 0xA2, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA2 (¢)
[VIEWDATA] Input byte: 0xA3 (.)
[VIEWDATA] interpret_char: 0xA3, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA3 (£)
[VIEWDATA] Input byte: 0xA4 (.)
[VIEWDATA] interpret_char: 0xA4, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA4 (¤)
[VIEWDATA] Input byte: 0xA5 (.)
[VIEWDATA] interpret_char: 0xA5, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA5 (¥)
[VIEWDATA] Input byte: 0xA6 (.)
[VIEWDATA] interpret_char: 0xA6, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA6 (¦)
[VIEWDATA] Input byte: 0xA7 (.)
[VIEWDATA] interpret_char: 0xA7, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA7 (§)
[VIEWDATA] Input byte: 0xA8 (.)
[VIEWDATA] interpret_char: 0xA8, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA8 (¨)
[VIEWDATA] Input byte: 0xA9 (.)
[VIEWDATA] interpret_char: 0xA9, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xA9 (©)
[VIEWDATA] Input byte: 0xAA (.)
[VIEWDATA] interpret_char: 0xAA, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xAA (ª)
[VIEWDATA] Input byte: 0xAB (.)
[VIEWDATA] interpret_char: 0xAB, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xAB («)
[VIEWDATA] Input byte: 0xAC (.)
[VIEWDATA] interpret_char: 0xAC, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xAC (¬)
[VIEWDATA] Input byte: 0xAD (.)
[VIEWDATA] interpret_char: 0xAD, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xAD (­)
[VIEWDATA] Input byte: 0xAE (.)
[VIEWDATA] interpret_char: 0xAE, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xAE (®)
[VIEWDATA] Input byte: 0xAF (.)
[VIEWDATA] interpret_char: 0xAF, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xAF (¯)
[VIEWDATA] Input byte: 0xB0 (.)
[VIEWDATA] interpret_char: 0xB0, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB0 (°)
[VIEWDATA] Input byte: 0xB1 (.)
[VIEWDATA] interpret_char: 0xB1, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB1 (±)
[VIEWDATA] Input byte: 0xB2 (.)
[VIEWDATA] interpret_char: 0xB2, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB2 (²)
[VIEWDATA] Input byte: 0xB3 (.)
[VIEWDATA] interpret_char: 0xB3, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB3 (³)
[VIEWDATA] Input byte: 0xB4 (.)
[VIEWDATA] interpret_char: 0xB4, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB4 (´)
[VIEWDATA] Input byte: 0xB5 (.)
[VIEWDATA] interpret_char: 0xB5, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB5 (µ)
[VIEWDATA] Input byte: 0xB6 (.)
[VIEWDATA] interpret_char: 0xB6, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB6 (¶)
[VIEWDATA] Input byte: 0xB7 (.)
[VIEWDATA] interpret_char: 0xB7, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB7 (·)
[VIEWDATA] Input byte: 0xB8 (.)
[VIEWDATA] interpret_char: 0xB8, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB8 (¸)
[VIEWDATA] Input byte: 0xB9 (.)
[VIEWDATA] interpret_char: 0xB9, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xB9 (¹)
[VIEWDATA] Input byte: 0xBA (.)
[VIEWDATA] interpret_char: 0xBA, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xBA (º)
[VIEWDATA] Input byte: 0xBB (.)
[VIEWDATA] interpret_char: 0xBB, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xBB (»)
[VIEWDATA] Input byte: 0xBC (.)
[VIEWDATA] interpret_char: 0xBC, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xBC (¼)
[VIEWDATA] Input byte: 0xBD (.)
[VIEWDATA] interpret_char: 0xBD, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xBD (½)
[VIEWDATA] Input byte: 0xBE (.)
[VIEWDATA] interpret_char: 0xBE, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xBE (¾)
[VIEWDATA] Input byte: 0xBF (.)
[VIEWDATA] interpret_char: 0xBF, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xBF (¿)
[VIEWDATA] Input byte: 0xC0 (.)
[VIEWDATA] interpret_char: 0xC0, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC0 (À)
[VIEWDATA] Input byte: 0xC1 (.)
[VIEWDATA] interpret_char: 0xC1, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC1 (Á)
[VIEWDATA] Input byte: 0xC2 (.)
[VIEWDATA] interpret_char: 0xC2, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC2 (Â)
[VIEWDATA] Input byte: 0xC3 (.)
[VIEWDATA] interpret_char: 0xC3, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC3 (Ã)
[VIEWDATA] Input byte: 0xC4 (.)
[VIEWDATA] interpret_char: 0xC4, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC4 (Ä)
[VIEWDATA] Input byte: 0xC5 (.)
[VIEWDATA] interpret_char: 0xC5, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC5 (Å)
[VIEWDATA] Input byte: 0xC6 (.)
[VIEWDATA] interpret_char: 0xC6, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC6 (Æ)
[VIEWDATA] Input byte: 0xC7 (.)
[VIEWDATA] interpret_char: 0xC7, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC7 (Ç)
[VIEWDATA] Input byte: 0xC8 (.)
[VIEWDATA] interpret_char: 0xC8, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC8 (È)
[VIEWDATA] Input byte: 0xC9 (.)
[VIEWDATA] interpret_char: 0xC9, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xC9 (É)
[VIEWDATA] Input byte: 0xCA (.)
[VIEWDATA] interpret_char: 0xCA, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xCA (Ê)
[VIEWDATA] Input byte: 0xCB (.)
[VIEWDATA] interpret_char: 0xCB, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xCB (Ë)
[VIEWDATA] Input byte: 0xCC (.)
[VIEWDATA] interpret_char: 0xCC, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xCC (Ì)
[VIEWDATA] Input byte: 0xCD (.)
[VIEWDATA] interpret_char: 0xCD, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xCD (Í)
[VIEWDATA] Input byte: 0xCE (.)
[VIEWDATA] interpret_char: 0xCE, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xCE (Î)
[VIEWDATA] Input byte: 0xCF (.)
[VIEWDATA] interpret_char: 0xCF, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xCF (Ï)
[VIEWDATA] Input byte: 0xD0 (.)
[VIEWDATA] interpret_char: 0xD0, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD0 (Ð)
[VIEWDATA] Input byte: 0xD1 (.)
[VIEWDATA] interpret_char: 0xD1, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD1 (Ñ)
[VIEWDATA] Input byte: 0xD2 (.)
[VIEWDATA] interpret_char: 0xD2, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD2 (Ò)
[VIEWDATA] Input byte: 0xD3 (.)
[VIEWDATA] interpret_char: 0xD3, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD3 (Ó)
[VIEWDATA] Input byte: 0xD4 (.)
[VIEWDATA] interpret_char: 0xD4, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD4 (Ô)
[VIEWDATA] Input byte: 0xD5 (.)
[VIEWDATA] interpret_char: 0xD5, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD5 (Õ)
[VIEWDATA] Input byte: 0xD6 (.)
[VIEWDATA] interpret_char: 0xD6, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD6 (Ö)
[VIEWDATA] Input byte: 0xD7 (.)
[VIEWDATA] interpret_char: 0xD7, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD7 (×)
[VIEWDATA] Input byte: 0xD8 (.)
[VIEWDATA] interpret_char: 0xD8, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD8 (Ø)
[VIEWDATA] Input byte: 0xD9 (.)
[VIEWDATA] interpret_char: 0xD9, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xD9 (Ù)
[VIEWDATA] Input byte: 0xDA (.)
[VIEWDATA] interpret_char: 0xDA, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xDA (Ú)
[VIEWDATA] Input byte: 0xDB (.)
[VIEWDATA] interpret_char: 0xDB, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xDB (Û)
[VIEWDATA] Input byte: 0xDC (.)
[VIEWDATA] interpret_char: 0xDC, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xDC (Ü)
[VIEWDATA] Input byte: 0xDD (.)
[VIEWDATA] interpret_char: 0xDD, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xDD (Ý)
[VIEWDATA] Input byte: 0xDE (.)
[VIEWDATA] interpret_char: 0xDE, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xDE (Þ)
[VIEWDATA] Input byte: 0xDF (.)
[VIEWDATA] interpret_char: 0xDF, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xDF (ß)
[VIEWDATA] Input byte: 0xE0 (.)
[VIEWDATA] interpret_char: 0xE0, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE0 (à)
[VIEWDATA] Input byte: 0xE1 (.)
[VIEWDATA] interpret_char: 0xE1, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE1 (á)
[VIEWDATA] Input byte: 0xE2 (.)
[VIEWDATA] interpret_char: 0xE2, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE2 (â)
[VIEWDATA] Input byte: 0xE3 (.)
[VIEWDATA] interpret_char: 0xE3, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE3 (ã)
[VIEWDATA] Input byte: 0xE4 (.)
[VIEWDATA] interpret_char: 0xE4, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE4 (ä)
[VIEWDATA] Input byte: 0xE5 (.)
[VIEWDATA] interpret_char: 0xE5, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE5 (å)
[VIEWDATA] Input byte: 0xE6 (.)
[VIEWDATA] interpret_char: 0xE6, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE6 (æ)
[VIEWDATA] Input byte: 0xE7 (.)
[VIEWDATA] interpret_char: 0xE7, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE7 (ç)
[VIEWDATA] Input byte: 0xE8 (.)
[VIEWDATA] interpret_char: 0xE8, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE8 (è)
[VIEWDATA] Input byte: 0xE9 (.)
[VIEWDATA] interpret_char: 0xE9, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xE9 (é)
[VIEWDATA] Input byte: 0xEA (.)
[VIEWDATA] interpret_char: 0xEA, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xEA (ê)
[VIEWDATA] Input byte: 0xEB (.)
[VIEWDATA] interpret_char: 0xEB, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xEB (ë)
[VIEWDATA] Input byte: 0xEC (.)
[VIEWDATA] interpret_char: 0xEC, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xEC (ì)
[VIEWDATA] Input byte: 0xED (.)
[VIEWDATA] interpret_char: 0xED, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xED (í)
[VIEWDATA] Input byte: 0xEE (.)
[VIEWDATA] interpret_char: 0xEE, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xEE (î)
[VIEWDATA] Input byte: 0xEF (.)
[VIEWDATA] interpret_char: 0xEF, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xEF (ï)
[VIEWDATA] Input byte: 0xF0 (.)
[VIEWDATA] interpret_char: 0xF0, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF0 (ð)
[VIEWDATA] Input byte: 0xF1 (.)
[VIEWDATA] interpret_char: 0xF1, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF1 (ñ)
[VIEWDATA] Input byte: 0xF2 (.)
[VIEWDATA] interpret_char: 0xF2, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF2 (ò)
[VIEWDATA] Input byte: 0xF3 (.)
[VIEWDATA] interpret_char: 0xF3, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF3 (ó)
[VIEWDATA] Input byte: 0xF4 (.)
[VIEWDATA] interpret_char: 0xF4, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF4 (ô)
[VIEWDATA] Input byte: 0xF5 (.)
[VIEWDATA] interpret_char: 0xF5, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF5 (õ)
[VIEWDATA] Input byte: 0xF6 (.)
[VIEWDATA] interpret_char: 0xF6, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF6 (ö)
[VIEWDATA] Input byte: 0xF7 (.)
[VIEWDATA] interpret_char: 0xF7, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF7 (÷)
[VIEWDATA] Input byte: 0xF8 (.)
[VIEWDATA] interpret_char: 0xF8, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF8 (ø)
[VIEWDATA] Input byte: 0xF9 (.)
[VIEWDATA] interpret_char: 0xF9, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xF9 (ù)
[VIEWDATA] Input byte: 0xFA (.)
[VIEWDATA] interpret_char: 0xFA, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xFA (ú)
[VIEWDATA] Input byte: 0xFB (.)
[VIEWDATA] interpret_char: 0xFB, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xFB (û)
[VIEWDATA] Input byte: 0xFC (.)
[VIEWDATA] interpret_char: 0xFC, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xFC (ü)
[VIEWDATA] Input byte: 0xFD (.)
[VIEWDATA] interpret_char: 0xFD, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xFD (ý)
[VIEWDATA] Input byte: 0xFE (.)
[VIEWDATA] interpret_char: 0xFE, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xFE (þ)
[VIEWDATA] Input byte: 0xFF (.)
[VIEWDATA] interpret_char: 0xFF, got_esc=false, graphic_mode=false, hold=false
[VIEWDATA]   Alpha char: 0xFF (ÿ)

*/
