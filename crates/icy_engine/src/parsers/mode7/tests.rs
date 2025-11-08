/*
use crate::{Buffer, BufferParser, Caret, Screen, TextScreen, parsers::update_buffer};

fn _create_mode7_buffer<T: BufferParser>(parser: &mut T, input: &[u8]) -> TextScreen {
    let mut buf = TextScreen::new((40, 25));
    buf.terminal_state().is_terminal_buffer = true;
    let mut caret = Caret::default();

    update_buffer(&mut buf, parser, input);

    buf
}
*/
