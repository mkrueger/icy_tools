//! Renders an ANSI file to a PNG using the same path as the golden-image tests.
//!
//! Usage: cargo run -p icy_engine --example render_ansi -- <input.ans> [output.png] [--chunk N]
//!
//! `--chunk N` feeds the data in N-byte pieces, the way a connection delivers it,
//! instead of one whole-file parse.

use std::{env, fs::File, io::BufWriter, path::PathBuf};

use icy_engine::{Rectangle, ScreenMode, ScreenSink};
use icy_net::telnet::TerminalEmulation;

fn main() {
    let mut positional = Vec::new();
    let mut chunk = 0usize;
    let mut lf_expand = true;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--chunk" {
            chunk = args.next().and_then(|value| value.parse().ok()).unwrap_or(0);
        } else if arg == "--no-lf-expand" {
            lf_expand = false;
        } else {
            positional.push(arg);
        }
    }

    let Some(input) = positional.first().map(PathBuf::from) else {
        eprintln!("usage: render_ansi <input.ans> [output.png] [--chunk N]");
        std::process::exit(2);
    };
    let output = positional.get(1).map(PathBuf::from).unwrap_or_else(|| input.with_extension("output.png"));

    let raw = std::fs::read(&input).unwrap_or_else(|err| panic!("cannot read {}: {err}", input.display()));
    let data = icy_sauce::strip_sauce(&raw, icy_sauce::StripMode::All);

    let (mut screen, mut parser) = ScreenMode::Vga(80, 25).create_screen(TerminalEmulation::Ansi, None);
    let screen = &mut *screen;
    screen.terminal_state_mut().lf_expand = lf_expand;
    let mut sink = ScreenSink::new(screen);
    if chunk == 0 {
        parser.parse(&data, &mut sink);
    } else {
        for piece in data.chunks(chunk) {
            parser.parse(piece, &mut sink);
        }
    }
    for (error, level) in sink.diagnostics() {
        eprintln!("{level}: {error}");
    }

    let rect: Rectangle = screen.size().into();
    let (size, pixels) = screen.render_to_rgba(&rect.into());

    let caret = screen.caret_position();
    println!("screen {}x{} caret {},{}", screen.width(), screen.height(), caret.x, caret.y);

    let file = File::create(&output).unwrap_or_else(|err| panic!("cannot create {}: {err}", output.display()));
    let mut encoder = png::Encoder::new(BufWriter::new(file), size.width as u32, size.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&pixels).unwrap();

    println!("{} -> {} ({}x{})", input.display(), output.display(), size.width, size.height);
}
