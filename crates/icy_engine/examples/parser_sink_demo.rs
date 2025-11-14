use icy_engine::{Screen, ScreenSink, Size, TextPane, TextScreen};
use icy_parser_core::{AnsiParser, CommandParser};

fn main() {
    // Create a text screen buffer
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Create a sink adapter
    let mut sink = ScreenSink::new(&mut screen);

    // Create an ANSI parser
    let mut parser = AnsiParser::new();

    // Parse some ANSI sequences
    let input = b"\x1b[1;31mHello, \x1b[32mWorld!\x1b[0m\n";
    parser.parse(input, &mut sink);

    // Check for any callbacks (like music playback, beeps, etc.)
    let callbacks = sink.take_callbacks();
    if !callbacks.is_empty() {
        println!("\nReceived {} callbacks:", callbacks.len());
        for callback in callbacks {
            println!("  - {:?}", callback);
        }
    }

    // Print the result
    println!("\nParsed ANSI text:");
    for y in 0..3 {
        for x in 0..20 {
            let ch = sink.screen().get_char((x, y).into());
            print!("{}", ch.ch);
        }
        println!();
    }

    println!("\nCaret position: ({}, {})", sink.screen().caret().x, sink.screen().caret().y);
}
