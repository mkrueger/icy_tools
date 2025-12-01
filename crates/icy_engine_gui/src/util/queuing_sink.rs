use std::collections::VecDeque;

use icy_engine::ScreenSink;
use icy_parser_core::{
    AnsiMusic, CommandSink, DeviceControlString, ErrorLevel, IgsCommand, OperatingSystemCommand, ParseError, RipCommand, SkypixCommand,
    TerminalCommand as ParserCommand, TerminalRequest, ViewDataCommand,
};

/// Queued command for processing
#[derive(Debug, Clone)]
pub enum QueuedCommand {
    Print(Vec<u8>),
    Command(ParserCommand),
    Music(AnsiMusic),
    Rip(RipCommand),
    Skypix(SkypixCommand),
    Igs(IgsCommand),
    ViewData(ViewDataCommand),
    Bell,
    ResizeTerminal(u16, u16),
    TerminalRequest(TerminalRequest),
    DeviceControl(DeviceControlString),
    OperatingSystemCommand(OperatingSystemCommand),
    Aps(Vec<u8>),
}

impl QueuedCommand {
    /// Check if this command produces sound (music, bells, sound effects)
    pub fn is_sound(&self) -> bool {
        matches!(
            self,
            QueuedCommand::Music(_)
                | QueuedCommand::Bell
                | QueuedCommand::Igs(IgsCommand::BellsAndWhistles { .. })
                | QueuedCommand::Igs(IgsCommand::AlterSoundEffect { .. })
                | QueuedCommand::Igs(IgsCommand::StopAllSound)
                | QueuedCommand::Igs(IgsCommand::RestoreSoundEffect { .. })
                | QueuedCommand::Igs(IgsCommand::SetEffectLoops { .. })
                | QueuedCommand::Igs(IgsCommand::ChipMusic { .. })
                | QueuedCommand::Igs(IgsCommand::Noise { .. })
                | QueuedCommand::Igs(IgsCommand::LoadMidiBuffer { .. })
        )
    }

    /// Check if this command introduces a delay/pause
    pub fn is_delay(&self) -> bool {
        matches!(
            self,
            QueuedCommand::Igs(IgsCommand::Pause { .. }) | QueuedCommand::Skypix(SkypixCommand::Delay { .. })
        )
    }

    /// Check if command needs async processing (for peeking in inner loop)
    pub fn needs_async_processing(&self) -> bool {
        self.is_sound()
            || self.is_delay()
            || matches!(
                self,
                QueuedCommand::Igs(IgsCommand::AskIG { .. })
                    | QueuedCommand::Skypix(SkypixCommand::CrcTransfer { .. })
                    | QueuedCommand::DeviceControl(DeviceControlString::Sixel { .. })
                    | QueuedCommand::TerminalRequest(_)
                    | QueuedCommand::ResizeTerminal(_, _)
            )
    }

    /// Process a command that requires screen access
    /// Returns true if GrabScreen was encountered
    pub fn process_screen_command(self, screen_sink: &mut ScreenSink<'_>) -> bool {
        match self {
            QueuedCommand::Print(text) => {
                screen_sink.print(&text);
            }
            QueuedCommand::Command(parser_cmd) => {
                screen_sink.emit(parser_cmd);
            }
            QueuedCommand::Rip(rip_cmd) => {
                screen_sink.screen().mark_dirty();
                screen_sink.emit_rip(rip_cmd);
            }
            QueuedCommand::Skypix(skypix_cmd) => {
                screen_sink.screen().mark_dirty();
                screen_sink.emit_skypix(skypix_cmd);
            }
            QueuedCommand::ViewData(vd_cmd) => {
                screen_sink.emit_view_data(vd_cmd);
            }
            QueuedCommand::Igs(ref igs_cmd) => {
                screen_sink.screen().mark_dirty();
                let had_grab = matches!(igs_cmd, IgsCommand::GrabScreen { .. });
                screen_sink.emit_igs(igs_cmd.clone());
                return had_grab;
            }
            QueuedCommand::DeviceControl(dcs) => {
                screen_sink.device_control(dcs);
            }
            QueuedCommand::OperatingSystemCommand(osc) => {
                screen_sink.operating_system_command(osc);
            }
            QueuedCommand::Aps(data) => {
                screen_sink.aps(&data);
            }
            _ => {}
        }
        false
    }
}

/// Custom CommandSink that queues commands instead of executing them immediately
pub struct QueueingSink<'a> {
    command_queue: &'a mut VecDeque<QueuedCommand>,
}

impl<'a> QueueingSink<'a> {
    pub fn new(queue: &'a mut VecDeque<QueuedCommand>) -> Self {
        Self { command_queue: queue }
    }
}

impl CommandSink for QueueingSink<'_> {
    fn print(&mut self, text: &[u8]) {
        self.command_queue.push_back(QueuedCommand::Print(text.to_vec()));
    }

    fn emit(&mut self, cmd: ParserCommand) {
        match &cmd {
            ParserCommand::Bell => {
                self.command_queue.push_back(QueuedCommand::Bell);
            }
            ParserCommand::CsiResizeTerminal(height, width) => {
                self.command_queue.push_back(QueuedCommand::ResizeTerminal(*width, *height));
            }
            _ => {
                self.command_queue.push_back(QueuedCommand::Command(cmd));
            }
        }
    }

    fn play_music(&mut self, music: AnsiMusic) {
        self.command_queue.push_back(QueuedCommand::Music(music));
    }

    fn emit_rip(&mut self, cmd: RipCommand) {
        self.command_queue.push_back(QueuedCommand::Rip(cmd));
    }

    fn emit_skypix(&mut self, cmd: SkypixCommand) {
        self.command_queue.push_back(QueuedCommand::Skypix(cmd));
    }

    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.command_queue.push_back(QueuedCommand::Igs(cmd));
    }

    fn emit_view_data(&mut self, cmd: ViewDataCommand) -> bool {
        self.command_queue.push_back(QueuedCommand::ViewData(cmd));
        // Return false since we can't check row change here - it will be done when command is executed
        false
    }

    fn device_control(&mut self, dcs: DeviceControlString) {
        self.command_queue.push_back(QueuedCommand::DeviceControl(dcs));
    }

    fn operating_system_command(&mut self, osc: OperatingSystemCommand) {
        self.command_queue.push_back(QueuedCommand::OperatingSystemCommand(osc));
    }

    fn aps(&mut self, data: &[u8]) {
        self.command_queue.push_back(QueuedCommand::Aps(data.to_vec()));
    }

    fn report_error(&mut self, error: ParseError, _level: ErrorLevel) {
        log::error!("Parse Error:{:?}", error);
    }

    fn request(&mut self, request: TerminalRequest) {
        self.command_queue.push_back(QueuedCommand::TerminalRequest(request));
    }
}
