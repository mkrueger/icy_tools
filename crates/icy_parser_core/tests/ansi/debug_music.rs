//! Debug test for ANSI music parsing

use icy_parser_core::{AnsiMusic, AnsiParser, CommandParser, CommandSink, MusicOption, TerminalCommand};

struct DebugSink {
    commands: Vec<String>,
}

impl DebugSink {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }
}

impl CommandSink for DebugSink {
    fn print(&mut self, text: &[u8]) {
        self.commands.push(format!("print: {:?}", String::from_utf8_lossy(text)));
    }
    fn emit(&mut self, cmd: TerminalCommand) {
        self.commands.push(format!("emit: {:?}", std::mem::discriminant(&cmd)));
    }
    fn emit_rip(&mut self, _cmd: icy_parser_core::RipCommand) {
        self.commands.push("emit_rip".to_string());
    }
    fn emit_skypix(&mut self, _cmd: icy_parser_core::SkypixCommand) {
        self.commands.push("emit_skypix".to_string());
    }
    fn emit_igs(&mut self, _cmd: icy_parser_core::IgsCommand) {
        self.commands.push("emit_igs".to_string());
    }
    fn device_control(&mut self, _dcs: icy_parser_core::DeviceControlString) {
        self.commands.push("device_control".to_string());
    }
    fn operating_system_command(&mut self, _osc: icy_parser_core::OperatingSystemCommand) {
        self.commands.push("operating_system_command".to_string());
    }
    fn aps(&mut self, _data: &[u8]) {
        self.commands.push("aps".to_string());
    }
    fn play_music(&mut self, music: AnsiMusic) {
        self.commands.push(format!("play_music: {} actions", music.music_actions.len()));
    }
    fn report_error(&mut self, error: icy_parser_core::ParseError) {
        self.commands.push(format!("error: {:?}", error));
    }
}

#[test]
fn debug_music_parsing() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = DebugSink::new();

    // Parse byte by byte to see what happens
    let sequence = b"\x1B[NC\x0E";
    eprintln!("Parsing sequence: {:?}", sequence);

    parser.parse(sequence, &mut sink);

    eprintln!("\nCommands received:");
    for (i, cmd) in sink.commands.iter().enumerate() {
        eprintln!("  {}: {}", i, cmd);
    }
}
