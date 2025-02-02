use crate::{Buffer, CallbackAction, Caret, EngineResult, Size};

use super::{cmd::IgsCommands, CommandExecutor};

type IgsCommandVec = Vec<(IgsCommands, Vec<i32>)>;
struct TestExecutor {
    pub commands: Arc<Mutex<IgsCommandVec>>,
}

impl CommandExecutor for TestExecutor {
    fn get_resolution(&self) -> Size {
        todo!();
    }

    fn execute_command(
        &mut self,
        _buf: &mut Buffer,
        _caret: &mut Caret,
        command: IgsCommands,
        parameters: &[i32],
        _string_parameter: &str,
    ) -> EngineResult<CallbackAction> {
        self.commands.lock().unwrap().push((command, parameters.to_vec()));
        Ok(CallbackAction::NoUpdate)
    }
}

use std::sync::{Arc, Mutex};

use crate::{
    igs::Parser,
    parsers::{create_buffer, update_buffer_force},
    TextPane,
};

fn create_parser() -> (Arc<Mutex<IgsCommandVec>>, Parser) {
    let commands = Arc::new(Mutex::new(Vec::new()));
    let command_executor = Arc::new(Mutex::new(TestExecutor { commands: commands.clone() }));
    let igs_parser = Parser::new(command_executor);
    (commands, igs_parser)
}

#[test]
pub fn test_line_breaks() {
    let (commands, mut igs_parser) = create_parser();
    create_buffer(&mut igs_parser, b"G#?>0:\nG#?>0:");
    assert_eq!(2, commands.lock().unwrap().len());
}

#[test]
pub fn test_igs_version() {
    let (commands, mut igs_parser) = create_parser();
    create_buffer(&mut igs_parser, b"G#?>0:");
    assert_eq!(1, commands.lock().unwrap().len());
    assert_eq!(IgsCommands::AskIG, commands.lock().unwrap()[0].0);
    assert_eq!(vec![0], commands.lock().unwrap()[0].1);
}

#[test]
pub fn parse_two_commands() {
    let (commands, mut igs_parser) = create_parser();
    create_buffer(&mut igs_parser, b"G#?>0:?>0:");
    assert_eq!(2, commands.lock().unwrap().len());
}

#[test]
pub fn test_eol_marker() {
    let (commands, mut igs_parser) = create_parser();
    create_buffer(&mut igs_parser, b"G#?>_\r\n0:?>_\r\n0:");
    assert_eq!(2, commands.lock().unwrap().len());
}

#[test]
pub fn test_text_break_bug() {
    let (_, mut igs_parser) = create_parser();
    let (buf, _) = create_buffer(&mut igs_parser, b"G#W>20,50,Chain@L 0,0,300,190:W>253,_\n140,IG SUPPORT BOARD@");

    assert_eq!(' ', buf.get_char((0, 0)).ch);
}

#[test]
pub fn test_loop_parsing() {
    let (_, mut igs_parser) = create_parser();
    let (mut buf, mut caret) = create_buffer(&mut igs_parser, b"");
    update_buffer_force(&mut buf, &mut caret, &mut igs_parser, b"G#&>0,320,4,0,L,8,0,100,x,0:0,100,x,199:");
    assert_eq!(' ', buf.get_char((0, 0)).ch);
}

/*
#[test]
pub fn test_loop_bug() {
    let (commands, mut igs_parser) = create_parser();
    create_buffer(
        &mut igs_parser,
        b"G#&>639,_\n\r320,4,0,L,8,639,100,x,0:639,100,x,199:W>147,_\n\r100,The friendliest             BBS on earth!!!@",
    );
    assert_eq!(81, commands.lock().unwrap().len());
    assert_eq!(IgsCommands::WriteText, commands.lock().unwrap()[80].0);
    assert_eq!(vec![147, 100, 0], commands.lock().unwrap()[80].1);
}*/

#[test]
pub fn test_loop_bug2() {
    let (commands, mut igs_parser) = create_parser();
    create_buffer(&mut igs_parser, b"G#S>0,0,0,0:\r\nG#S>0,0,0,0:\r\nG#S>0,0,0,0:\r\nG#S>0,0,0,0:\r\n");
    assert_eq!(4, commands.lock().unwrap().len());
}

#[test]
pub fn test_polyline_bug() {
    let (commands, mut igs_parser) = create_parser();
    create_buffer(&mut igs_parser, b"G#z>3:28,29,62,113,129,45:\r\n");
    let cmd = &commands.lock().unwrap()[0];
    assert_eq!(IgsCommands::PolyLine, cmd.0);
    assert_eq!(vec![3, 28, 29, 62, 113, 129, 45], cmd.1);
}

#[test]
pub fn test_chain_gang_loop() {
    let (_commands, mut igs_parser) = create_parser();
    create_buffer(
        &mut igs_parser,
        b"G#&>1,10,1,0,>Gq@,22,0G3,3,0,102,20,107,218,156:1q10:0G3,3,0,109,20,114,218,156:1q10:\r\n",
    );
}
