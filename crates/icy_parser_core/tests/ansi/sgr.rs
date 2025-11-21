use super::*;

#[test]
fn test_sgr_reset() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[m - Reset (default, no parameter)
    parser.parse(b"\x1B[m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset)));

    sink.cmds.clear();

    // ESC[0m - Reset (explicit)
    parser.parse(b"\x1B[0m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset)));
}

#[test]
fn test_sgr_intensity() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[1m - Bold
    parser.parse(b"\x1B[1m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Bold))
    ));

    sink.cmds.clear();

    // ESC[2m - Faint/Dim
    parser.parse(b"\x1B[2m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Faint))
    ));

    sink.cmds.clear();

    // ESC[22m - Normal intensity (neither bold nor faint)
    parser.parse(b"\x1B[22m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Normal))
    ));
}

#[test]
fn test_sgr_italic_and_fraktur() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[3m - Italic on
    parser.parse(b"\x1B[3m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Italic(true))));

    sink.cmds.clear();

    // ESC[23m - Italic off (also turns off Fraktur)
    parser.parse(b"\x1B[23m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Italic(false))));

    sink.cmds.clear();

    // ESC[20m - Fraktur (Gothic font)
    parser.parse(b"\x1B[20m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Fraktur)));
}

#[test]
fn test_sgr_underline() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[4m - Single underline
    parser.parse(b"\x1B[4m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(Underline::Single))
    ));

    sink.cmds.clear();

    // ESC[21m - Double underline
    parser.parse(b"\x1B[21m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(Underline::Double))
    ));

    sink.cmds.clear();

    // ESC[24m - Underline off
    parser.parse(b"\x1B[24m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(Underline::Off))
    ));
}

#[test]
fn test_sgr_blink() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5m - Slow blink
    parser.parse(b"\x1B[5m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow))
    ));

    sink.cmds.clear();

    // ESC[6m - Rapid blink
    parser.parse(b"\x1B[6m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Rapid))
    ));

    sink.cmds.clear();

    // ESC[25m - Blink off
    parser.parse(b"\x1B[25m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Off))
    ));
}

#[test]
fn test_sgr_inverse_and_concealed() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[7m - Inverse on
    parser.parse(b"\x1B[7m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(true))));

    sink.cmds.clear();

    // ESC[27m - Inverse off
    parser.parse(b"\x1B[27m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(false))
    ));

    sink.cmds.clear();

    // ESC[8m - Concealed on
    parser.parse(b"\x1B[8m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(true))
    ));

    sink.cmds.clear();

    // ESC[28m - Concealed off (revealed)
    parser.parse(b"\x1B[28m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Concealed(false))
    ));
}

#[test]
fn test_sgr_crossed_out() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[9m - Crossed out on
    parser.parse(b"\x1B[9m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::CrossedOut(true))
    ));

    sink.cmds.clear();

    // ESC[29m - Crossed out off
    parser.parse(b"\x1B[29m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::CrossedOut(false))
    ));
}

#[test]
fn test_sgr_frame_and_overlined() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[51m - Framed
    parser.parse(b"\x1B[51m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Frame(Frame::Framed))
    ));

    sink.cmds.clear();

    // ESC[52m - Encircled
    parser.parse(b"\x1B[52m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Frame(Frame::Encircled))
    ));

    sink.cmds.clear();

    // ESC[54m - Frame/Encircle off
    parser.parse(b"\x1B[54m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Frame(Frame::Off))
    ));

    sink.cmds.clear();

    // ESC[53m - Overlined on
    parser.parse(b"\x1B[53m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Overlined(true))
    ));

    sink.cmds.clear();

    // ESC[55m - Overlined off
    parser.parse(b"\x1B[55m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Overlined(false))
    ));
}

#[test]
fn test_sgr_fonts() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[10m - Primary font (font 0)
    parser.parse(b"\x1B[10m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Font(0))));

    sink.cmds.clear();

    // ESC[11m - Alternative font 1
    parser.parse(b"\x1B[11m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Font(1))));

    sink.cmds.clear();

    // ESC[15m - Alternative font 5
    parser.parse(b"\x1B[15m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Font(5))));

    sink.cmds.clear();

    // ESC[19m - Alternative font 9
    parser.parse(b"\x1B[19m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Font(9))));
}

#[test]
fn test_sgr_base_colors() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[30m - Black foreground
    parser.parse(b"\x1B[30m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(0)))
    ));

    sink.cmds.clear();

    // ESC[31m - Red foreground
    parser.parse(b"\x1B[31m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(4)))
    ));

    sink.cmds.clear();

    // ESC[37m - White foreground
    parser.parse(b"\x1B[37m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(7)))
    ));

    sink.cmds.clear();

    // ESC[39m - Default foreground
    parser.parse(b"\x1B[39m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Default))
    ));

    sink.cmds.clear();

    // ESC[40m - Black background
    parser.parse(b"\x1B[40m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0)))
    ));

    sink.cmds.clear();

    // ESC[44m - Blue background
    parser.parse(b"\x1B[44m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(1)))
    ));

    sink.cmds.clear();

    // ESC[49m - Default background
    parser.parse(b"\x1B[49m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Default))
    ));
}

#[test]
fn test_sgr_bright_colors() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[90m - Bright black (gray) foreground
    parser.parse(b"\x1B[90m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(8)))
    ));

    sink.cmds.clear();

    // ESC[91m - Bright red foreground
    parser.parse(b"\x1B[91m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(12)))
    ));

    sink.cmds.clear();

    // ESC[97m - Bright white foreground
    parser.parse(b"\x1B[97m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(15)))
    ));

    sink.cmds.clear();

    // ESC[100m - Bright black background
    parser.parse(b"\x1B[100m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(8)))
    ));

    sink.cmds.clear();

    // ESC[102m - Bright green background
    parser.parse(b"\x1B[102m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(10)))
    ));

    sink.cmds.clear();

    // ESC[91;102m - Bright red foreground + Bright green background (emits 2 commands)
    parser.parse(b"\x1B[91;102m", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(12)))
    ));
    assert!(matches!(
        &sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(10)))
    ));
}

#[test]
fn test_sgr_extended_colors() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[38;5;123m - 256-color foreground
    parser.parse(b"\x1B[38;5;123m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Extended(123)))
    ));

    sink.cmds.clear();

    // ESC[48;5;200m - 256-color background
    parser.parse(b"\x1B[48;5;200m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Extended(200)))
    ));

    sink.cmds.clear();

    // ESC[38;2;255;128;64m - RGB foreground
    parser.parse(b"\x1B[38;2;255;128;64m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Rgb(255, 128, 64)))
    ));

    sink.cmds.clear();

    // ESC[48;2;100;150;200m - RGB background
    parser.parse(b"\x1B[48;2;100;150;200m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Rgb(100, 150, 200)))
    ));
}

#[test]
fn test_sgr_ideogram_attributes() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[60m - Ideogram underline or right side line
    parser.parse(b"\x1B[60m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::IdeogramUnderline)
    ));

    sink.cmds.clear();

    // ESC[61m - Ideogram double underline or double line on right side
    parser.parse(b"\x1B[61m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::IdeogramDoubleUnderline)
    ));

    sink.cmds.clear();

    // ESC[62m - Ideogram overline or left side line
    parser.parse(b"\x1B[62m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::IdeogramOverline)
    ));

    sink.cmds.clear();

    // ESC[63m - Ideogram double overline or double line on left side
    parser.parse(b"\x1B[63m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::IdeogramDoubleOverline)
    ));

    sink.cmds.clear();

    // ESC[64m - Ideogram stress marking
    parser.parse(b"\x1B[64m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::IdeogramStress)
    ));

    sink.cmds.clear();

    // ESC[65m - Cancel ideogram attributes
    parser.parse(b"\x1B[65m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::IdeogramAttributesOff)
    ));
}

#[test]
fn test_sgr_combined_attributes() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[1;31m - Bold + Red foreground (emits 2 separate commands)
    parser.parse(b"\x1B[1;31m", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Bold))
    ));
    assert!(matches!(
        &sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(4)))
    ));

    sink.cmds.clear();

    // ESC[3;4;9m - Italic + Underline + CrossedOut (emits 3 commands)
    parser.parse(b"\x1B[3;4;9m", &mut sink);
    assert_eq!(sink.cmds.len(), 3);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Italic(true))));
    assert!(matches!(
        &sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(Underline::Single))
    ));
    assert!(matches!(
        &sink.cmds[2],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::CrossedOut(true))
    ));

    sink.cmds.clear();

    // ESC[5;7m - SlowBlink + Inverse (emits 2 commands)
    parser.parse(b"\x1B[5;7m", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow))
    ));
    assert!(matches!(&sink.cmds[1], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(true))));
}
