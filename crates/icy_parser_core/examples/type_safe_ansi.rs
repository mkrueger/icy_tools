//! Example demonstrating type-safe ANSI command parameters and error reporting

use icy_parser_core::{
    AnsiMode, AnsiParser, Blink, Color, CommandParser, CommandSink, DecPrivateMode, DeviceStatusReport, EraseInDisplayMode, EraseInLineMode, Intensity,
    ParseError, SgrAttribute, TerminalCommand, Underline,
};

struct ExampleSink {
    command_count: usize,
    error_count: usize,
}

impl CommandSink for ExampleSink {
    fn print(&mut self, text: &[u8]) {
        self.command_count += 1;
        println!("  [{}] Printable: {:?}", self.command_count, String::from_utf8_lossy(text));
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.command_count += 1;
        match cmd {
            TerminalCommand::CsiEraseInDisplay(mode) => {
                println!("  [{}] Erase in Display: {:?}", self.command_count, mode);
                match mode {
                    EraseInDisplayMode::CursorToEnd => {
                        println!("      → Clear from cursor to end of display")
                    }
                    EraseInDisplayMode::StartToCursor => {
                        println!("      → Clear from start to cursor")
                    }
                    EraseInDisplayMode::All => println!("      → Clear entire display"),
                    EraseInDisplayMode::AllAndScrollback => {
                        println!("      → Clear display and scrollback")
                    }
                }
            }
            TerminalCommand::CsiEraseInLine(mode) => {
                println!("  [{}] Erase in Line: {:?}", self.command_count, mode);
                match mode {
                    EraseInLineMode::CursorToEnd => {
                        println!("      → Clear from cursor to end of line")
                    }
                    EraseInLineMode::StartToCursor => {
                        println!("      → Clear from start of line to cursor")
                    }
                    EraseInLineMode::All => println!("      → Clear entire line"),
                }
            }
            TerminalCommand::CsiDeviceStatusReport(report) => {
                println!("  [{}] Device Status Report: {:?}", self.command_count, report);
                match report {
                    DeviceStatusReport::OperatingStatus => println!("      → Request operating status"),
                    DeviceStatusReport::CursorPosition => println!("      → Request cursor position"),
                }
            }
            TerminalCommand::CsiSetMode(mode) => {
                println!("  [{}] Set Mode:", self.command_count);
                match mode {
                    AnsiMode::InsertReplace => println!("      → Insert/Replace Mode (IRM) - insert mode ON"),
                }
            }
            TerminalCommand::CsiResetMode(mode) => {
                println!("  [{}] Reset Mode:", self.command_count);
                match mode {
                    AnsiMode::InsertReplace => println!("      → Insert/Replace Mode (IRM) - insert mode OFF"),
                }
            }
            TerminalCommand::CsiDecPrivateModeSet(mode) => {
                println!("  [{}] DEC Private Mode Set:", self.command_count);
                match mode {
                    DecPrivateMode::CursorVisible => println!("      → Cursor Visible (DECTCEM)"),
                    DecPrivateMode::AutoWrap => println!("      → Auto Wrap (DECAWM)"),
                    DecPrivateMode::VT200Mouse => println!("      → VT200 Mouse Tracking"),
                    DecPrivateMode::IceColors => println!("      → iCE Colors (background intensity)"),
                    other => println!("      → {:?}", other),
                }
            }
            TerminalCommand::CsiDecPrivateModeReset(mode) => {
                println!("  [{}] DEC Private Mode Reset:", self.command_count);
                match mode {
                    DecPrivateMode::CursorVisible => println!("      → Cursor Invisible (DECTCEM)"),
                    DecPrivateMode::AutoWrap => println!("      → No Auto Wrap (DECAWM)"),
                    DecPrivateMode::VT200Mouse => println!("      → Disable VT200 Mouse"),
                    DecPrivateMode::IceColors => println!("      → Standard Blink Mode"),
                    other => println!("      → {:?}", other),
                }
            }
            TerminalCommand::CsiCursorPosition(row, col) => {
                println!("  [{}] Cursor Position: row={}, col={}", self.command_count, row, col);
            }
            TerminalCommand::CsiSelectGraphicRendition(attr) => {
                print!("  [{}] SGR: ", self.command_count);
                match attr {
                    SgrAttribute::Reset => println!("Reset all attributes"),
                    SgrAttribute::Intensity(Intensity::Bold) => println!("Bold / Increased Intensity"),
                    SgrAttribute::Intensity(Intensity::Faint) => println!("Faint / Decreased Intensity"),
                    SgrAttribute::Intensity(Intensity::Normal) => println!("Normal Intensity"),
                    SgrAttribute::Italic(true) => println!("Italic ON"),
                    SgrAttribute::Italic(false) => println!("Italic OFF"),
                    SgrAttribute::Underline(Underline::Single) => println!("Underline Single"),
                    SgrAttribute::Underline(Underline::Double) => println!("Underline Double"),
                    SgrAttribute::Underline(Underline::Off) => println!("Underline OFF"),
                    SgrAttribute::Blink(Blink::Slow) => println!("Slow Blink"),
                    SgrAttribute::Blink(Blink::Rapid) => println!("Rapid Blink"),
                    SgrAttribute::Blink(Blink::Off) => println!("Blink OFF (steady)"),
                    SgrAttribute::Inverse(true) => println!("Inverse ON (swap fg/bg)"),
                    SgrAttribute::Inverse(false) => println!("Inverse OFF"),
                    SgrAttribute::Concealed(true) => println!("Concealed ON"),
                    SgrAttribute::Concealed(false) => println!("Concealed OFF (revealed)"),
                    SgrAttribute::CrossedOut(true) => println!("Crossed Out ON"),
                    SgrAttribute::CrossedOut(false) => println!("Crossed Out OFF"),
                    SgrAttribute::Font(n) => println!("Font {} (0=default, 1-9=alternative)", n),
                    SgrAttribute::Foreground(color) => match color {
                        Color::Base(0) => println!("Foreground: Black"),
                        Color::Base(1) => println!("Foreground: Red"),
                        Color::Base(2) => println!("Foreground: Green"),
                        Color::Base(3) => println!("Foreground: Yellow"),
                        Color::Base(4) => println!("Foreground: Blue"),
                        Color::Base(5) => println!("Foreground: Magenta"),
                        Color::Base(6) => println!("Foreground: Cyan"),
                        Color::Base(7) => println!("Foreground: White"),
                        Color::Base(8) => println!("Foreground: Bright Black"),
                        Color::Base(9) => println!("Foreground: Bright Red"),
                        Color::Base(10) => println!("Foreground: Bright Green"),
                        Color::Base(11) => println!("Foreground: Bright Yellow"),
                        Color::Base(12) => println!("Foreground: Bright Blue"),
                        Color::Base(13) => println!("Foreground: Bright Magenta"),
                        Color::Base(14) => println!("Foreground: Bright Cyan"),
                        Color::Base(15) => println!("Foreground: Bright White"),
                        Color::Base(n) => println!("Foreground: Base #{}", n),
                        Color::Extended(n) => println!("Foreground: 256-color palette #{}", n),
                        Color::Rgb(r, g, b) => println!("Foreground: RGB({}, {}, {})", r, g, b),
                        Color::Default => println!("Foreground: Default"),
                    },
                    SgrAttribute::Background(color) => match color {
                        Color::Base(0) => println!("Background: Black"),
                        Color::Base(1) => println!("Background: Red"),
                        Color::Base(2) => println!("Background: Green"),
                        Color::Base(3) => println!("Background: Yellow"),
                        Color::Base(4) => println!("Background: Blue"),
                        Color::Base(5) => println!("Background: Magenta"),
                        Color::Base(6) => println!("Background: Cyan"),
                        Color::Base(7) => println!("Background: White"),
                        Color::Base(8) => println!("Background: Bright Black"),
                        Color::Base(9) => println!("Background: Bright Red"),
                        Color::Base(10) => println!("Background: Bright Green"),
                        Color::Base(11) => println!("Background: Bright Yellow"),
                        Color::Base(12) => println!("Background: Bright Blue"),
                        Color::Base(13) => println!("Background: Bright Magenta"),
                        Color::Base(14) => println!("Background: Bright Cyan"),
                        Color::Base(15) => println!("Background: Bright White"),
                        Color::Base(n) => println!("Background: Base #{}", n),
                        Color::Extended(n) => println!("Background: 256-color palette #{}", n),
                        Color::Rgb(r, g, b) => println!("Background: RGB({}, {}, {})", r, g, b),
                        Color::Default => println!("Background: Default"),
                    },
                    other => println!("{:?}", other),
                }
            }
            other => {
                println!("  [{}] {:?}", self.command_count, other);
            }
        }
    }

    fn report_error(&mut self, error: ParseError) {
        self.error_count += 1;
        eprintln!("⚠️  Parse Error #{}: {:?}", self.error_count, error);
        match error {
            ParseError::InvalidParameter { command, value } => {
                eprintln!("    Command '{}' received invalid parameter: {}", command, value);
            }
            ParseError::IncompleteSequence => {
                eprintln!("    Incomplete escape sequence at end of input");
            }
            ParseError::MalformedSequence { description } => {
                eprintln!("    Malformed sequence: {}", description);
            }
        }
    }
}

fn main() {
    let mut parser = AnsiParser::new();
    let mut sink = ExampleSink {
        command_count: 0,
        error_count: 0,
    };

    println!("=== Valid ANSI Commands ===\n");

    // Valid erase commands
    println!("Input: ESC[2J (clear entire display)");
    parser.parse(b"\x1B[2J", &mut sink);

    println!("\nInput: ESC[1K (clear from start of line to cursor)");
    parser.parse(b"\x1B[1K", &mut sink);

    println!("\nInput: ESC[6n (device status report - cursor position)");
    parser.parse(b"\x1B[6n", &mut sink);

    println!("\nInput: ESC[10;20H (cursor position row 10, col 20)");
    parser.parse(b"\x1B[10;20H", &mut sink);

    println!("\nInput: ESC[4h (set Insert/Replace mode)");
    parser.parse(b"\x1B[4h", &mut sink);

    println!("\nInput: ESC[4l (reset Insert/Replace mode)");
    parser.parse(b"\x1B[4l", &mut sink);

    println!("\nInput: ESC[?25h (DECSET - show cursor)");
    parser.parse(b"\x1B[?25h", &mut sink);

    println!("\nInput: ESC[?7l (DECRST - disable auto wrap)");
    parser.parse(b"\x1B[?7l", &mut sink);

    println!("\nInput: ESC[?25;1000h (multiple DEC modes: cursor + mouse)");
    parser.parse(b"\x1B[?25;1000h", &mut sink);

    // SGR commands
    println!("\nInput: ESC[1;31m (bold + red foreground)");
    parser.parse(b"\x1B[1;31m", &mut sink);

    println!("\nInput: ESC[3;4;9m (italic + underline + crossed out)");
    parser.parse(b"\x1B[3;4;9m", &mut sink);

    println!("\nInput: ESC[38;5;123m (256-color foreground palette #123)");
    parser.parse(b"\x1B[38;5;123m", &mut sink);

    println!("\nInput: ESC[38;2;255;128;64m (RGB foreground: orange)");
    parser.parse(b"\x1B[38;2;255;128;64m", &mut sink);

    println!("\nInput: ESC[91;102m (bright red fg + bright green bg)");
    parser.parse(b"\x1B[91;102m", &mut sink);

    println!("\nInput: ESC[m (reset - implicit 0)");
    parser.parse(b"\x1B[m", &mut sink);

    println!("\n=== Invalid Parameters (will trigger error reporting) ===\n");

    // Invalid parameters
    println!("Input: ESC[99J (invalid erase in display - valid range is 0-3)");
    parser.parse(b"\x1B[99J", &mut sink);

    println!("\nInput: ESC[5K (invalid erase in line - valid range is 0-2)");
    parser.parse(b"\x1B[5K", &mut sink);

    println!("\nInput: ESC[99n (invalid device status report - valid values are 5 or 6)");
    parser.parse(b"\x1B[99n", &mut sink);

    println!("\nInput: ESC[99h (invalid mode - valid ANSI mode is 4 only)");
    parser.parse(b"\x1B[99h", &mut sink);

    println!("\nInput: ESC[4;99;4h (mixed valid and invalid modes)");
    parser.parse(b"\x1B[4;99;4h", &mut sink);

    println!("\nInput: ESC[?9999h (invalid DEC private mode)");
    parser.parse(b"\x1B[?9999h", &mut sink);

    println!("\nInput: ESC[?25;9999;1000h (mixed valid and invalid DEC private modes)");
    parser.parse(b"\x1B[?25;9999;1000h", &mut sink);

    println!("\n=== Summary ===");
    println!("Total commands processed: {}", sink.command_count);
    println!("Total errors reported: {}", sink.error_count);
}
