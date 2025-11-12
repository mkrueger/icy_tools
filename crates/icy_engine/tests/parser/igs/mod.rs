mod vdi;
/*
use icy_engine::{BufferParser, Caret, Position, SelectionMask, TextBuffer, TextPane, TextScreen};
use icy_engine::parsers::igs::Parser;

fn create_buffer<T: BufferParser>(parser: &mut T, input: &[u8]) -> (TextBuffer, Caret) {
    let mut screen = TextScreen {
        buffer: TextBuffer::create((80, 25)),
        caret: Caret::default(),
        current_layer: 0,
        selection_opt: None,
        selection_mask: SelectionMask::default(),
        mouse_fields: Vec::new(),
    };

    for &b in input {
        parser.print_char(&mut screen, b as char).unwrap();
    }

    while parser.get_next_action(&mut screen).is_some() {}

    (screen.buffer, screen.caret)
}

fn update_buffer_force<T: BufferParser>(buf: &mut TextBuffer, caret: &mut Caret, parser: &mut T, input: &[u8]) {
    let mut screen = TextScreen {
        buffer: std::mem::take(buf),
        caret: caret.clone(),
        current_layer: 0,
        selection_opt: None,
        selection_mask: SelectionMask::default(),
        mouse_fields: Vec::new(),
    };

    for &b in input {
        parser.print_char(&mut screen, b as char).unwrap();
    }

    while parser.get_next_action(&mut screen).is_some() {}

    *buf = screen.buffer;
    *caret = screen.caret;
}

#[test]
pub fn test_text_break_bug() {
    let mut igs_parser: Parser = Parser::new(icy_engine::igs::TerminalResolution::Low);
    let (buf, _) = create_buffer(&mut igs_parser, b"G#W>20,50,Chain@L 0,0,300,190:W>253,_\n140,IG SUPPORT BOARD@");

    assert_eq!(' ', buf.get_char(Position::new(0, 0)).ch);
}

#[test]
pub fn test_loop_parsing() {
    let mut igs_parser = Parser::new(icy_engine::igs::TerminalResolution::Low);
    let (mut buf, mut caret) = create_buffer(&mut igs_parser, b"");
    update_buffer_force(&mut buf, &mut caret, &mut igs_parser, b"G#&>0,320,4,0,L,8,0,100,x,0:0,100,x,199:");
    assert_eq!(' ', buf.get_char(Position::new(0, 0)).ch);
}

#[test]
pub fn test_chain_gang_loop() {
    let mut igs_parser = Parser::new(icy_engine::igs::TerminalResolution::Low);
    create_buffer(
        &mut igs_parser,
        b"G#&>1,10,1,0,>Gq@,22,0G3,3,0,102,20,107,218,156:1q10:0G3,3,0,109,20,114,218,156:1q10:\r\n",
    );
}*/
