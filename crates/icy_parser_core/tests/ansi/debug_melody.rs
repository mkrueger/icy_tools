use icy_parser_core::ErrorLevel;

#[test]
fn debug_melody() {
    use icy_parser_core::{
        AnsiMusic, AnsiParser, CommandParser, CommandSink, DeviceControlString, IgsCommand, MusicOption, OperatingSystemCommand, ParseError, RipCommand,
        SkypixCommand, TerminalCommand,
    };

    struct DebugSink {
        _commands: Vec<String>,
    }

    impl DebugSink {
        fn new() -> Self {
            Self { _commands: Vec::new() }
        }
    }

    impl CommandSink for DebugSink {
        fn print(&mut self, text: &[u8]) {
            eprintln!("print: {:?}", String::from_utf8_lossy(text));
        }
        fn emit(&mut self, cmd: TerminalCommand) {
            eprintln!("emit: {:?}", std::mem::discriminant(&cmd));
        }
        fn emit_rip(&mut self, _cmd: RipCommand) {}
        fn emit_skypix(&mut self, _cmd: SkypixCommand) {}
        fn emit_igs(&mut self, _cmd: IgsCommand) {}
        fn device_control(&mut self, _dcs: DeviceControlString) {}
        fn operating_system_command(&mut self, _osc: OperatingSystemCommand) {}
        fn aps(&mut self, _data: &[u8]) {}
        fn play_music(&mut self, music: AnsiMusic) {
            eprintln!("play_music: {} actions", music.music_actions.len());
            for (i, action) in music.music_actions.iter().enumerate() {
                eprintln!("  Action {}: {:?}", i, action);
            }
        }
        fn report_errror(&mut self, error: ParseError, _level: ErrorLevel) {
            eprintln!("error: {:?}", error);
        }
    }

    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = DebugSink::new();

    eprintln!("\nParsing: \\x1B[MT225O3L8GL8GL8GL2E-P8L8FL8FL8FL2D\\x0E");
    parser.parse(b"\x1B[MT225O3L8GL8GL8GL2E-P8L8FL8FL8FL2D\x0E", &mut sink);
}
