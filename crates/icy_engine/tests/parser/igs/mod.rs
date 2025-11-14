mod vdi;

use icy_engine::parsers::igs::Parser;
use icy_engine::{ATARI, BitFont, BufferParser, Caret, EditableScreen, IGS_SYSTEM_PALETTE, Palette, PaletteScreenBuffer};

fn create_buffer<T: BufferParser>(parser: &mut T, input: &[u8]) -> (PaletteScreenBuffer, Caret) {
    let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::IGS(icy_engine::igs::TerminalResolution::Low));
    *buffer.palette_mut() = Palette::from_slice(&IGS_SYSTEM_PALETTE);

    for &b in input {
        parser.print_char(&mut buffer, b as char).unwrap();
    }

    while parser.get_next_action(&mut buffer).is_some() {}

    let caret = Caret::default(); // IGS doesn't use text caret
    (buffer, caret)
}

fn update_buffer_force<T: BufferParser>(buf: &mut PaletteScreenBuffer, _caret: &mut Caret, parser: &mut T, input: &[u8]) {
    for &b in input {
        parser.print_char(buf, b as char).unwrap();
    }

    while parser.get_next_action(buf).is_some() {}
}

#[test]
pub fn test_text_break_bug() {
    let mut igs_parser: Parser = Parser::new(icy_engine::igs::TerminalResolution::Low);
    let (_buf, _) = create_buffer(&mut igs_parser, b"G#W>20,50,Chain@L 0,0,300,190:W>253,_\n140,IG SUPPORT BOARD@");

    // Just checking that parsing doesn't crash
}

#[test]
pub fn test_loop_parsing() {
    let mut igs_parser = Parser::new(icy_engine::igs::TerminalResolution::Low);
    let (mut buf, mut caret) = create_buffer(&mut igs_parser, b"");
    update_buffer_force(&mut buf, &mut caret, &mut igs_parser, b"G#&>0,320,4,0,L,8,0,100,x,0:0,100,x,199:");
    // Just checking that parsing doesn't crash
}

#[test]
pub fn test_chain_gang_loop() {
    let mut igs_parser = Parser::new(icy_engine::igs::TerminalResolution::Low);
    create_buffer(
        &mut igs_parser,
        b"G#&>1,10,1,0,>Gq@,22,0G3,3,0,102,20,107,218,156:1q10:0G3,3,0,109,20,114,218,156:1q10:\r\n",
    );
}
