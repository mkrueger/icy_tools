#![cfg(test)]
#![allow(dead_code, unused_variables, unused_imports)]

//! Tests for ANSI parser features that require CallbackAction support
//!
//! These tests check terminal features that need to send responses back to the host:
//! - ANSI music (CSI N commands)
//! - Baud emulation (CSI *r commands)
//! - Terminal reports (device attributes, cursor position, tab stops, etc.)
//! - Rectangular area checksums (DECRQCRA)
//! - DCS macro reports
//!
//! These tests are currently disabled as they require implementing a callback
//! mechanism in the new parser architecture. They should be ported to IcyTerm
//! or implemented with a new callback sink.

/*
TODO: These tests need to be converted to use the new parser architecture.
The old parser had a CallbackAction return type that the new CommandSink doesn't support.
These need either:
1. A new sink that captures callbacks
2. Moving to IcyTerm where callbacks are handled
3. A different testing approach for terminal responses

#[test]
fn test_music() {
    let mut p = ansi::Parser::default();
    p.ansi_music = MusicOption::Both;

    let action = get_simple_action(&mut p, b"\x1B[NC\x0E");
    let CallbackAction::PlayMusic(music) = action else {
        panic!();
    };
    assert_eq!(1, music.music_actions.len());
    let MusicAction::PlayNote(f, len, _) = music.music_actions[0] else {
        panic!();
    };
    assert_eq!(523.2511, f);
    assert_eq!(4 * 120, len);
}

#[test]
fn test_set_length() {
    let mut p = ansi::Parser::default();
    p.ansi_music = MusicOption::Both;

    let action = get_simple_action(&mut p, b"\x1B[NNL8C\x0E");
    let CallbackAction::PlayMusic(music) = action else {
        panic!();
    };
    assert_eq!(2, music.music_actions.len());
    let MusicAction::PlayNote(f, len, _) = music.music_actions[1] else {
        panic!();
    };
    assert_eq!(523.2511, f);
    assert_eq!(8 * 120, len);
}

#[test]
fn test_tempo() {
    let mut p = ansi::Parser::default();
    p.ansi_music = MusicOption::Both;

    let action = get_simple_action(&mut p, b"\x1B[NT123C\x0E");
    let CallbackAction::PlayMusic(music) = action else {
        panic!();
    };
    assert_eq!(1, music.music_actions.len());
}

#[test]
fn test_pause() {
    let mut p = ansi::Parser::default();
    p.ansi_music = MusicOption::Both;

    let action = get_simple_action(&mut p, b"\x1B[NP32.\x0E");
    let CallbackAction::PlayMusic(music) = action else {
        panic!();
    };
    assert_eq!(1, music.music_actions.len());
    let MusicAction::Pause(t) = music.music_actions[0] else {
        panic!();
    };
    assert_eq!(5760, t);
}

#[test]
fn test_melody() {
    let mut p = ansi::Parser::default();
    p.ansi_music = MusicOption::Both;

    let action = get_simple_action(&mut p, b"\x1B[MFT225O3L8GL8GL8GL2E-P8L8FL8FL8FMLL2DL2DMNP8\x0E");
    let CallbackAction::PlayMusic(music) = action else {
        panic!();
    };
    assert_eq!(14, music.music_actions.len());
}

#[test]
fn test_select_communication_speed() {
    let (mut buf, mut caret) = create_buffer(b"");
    assert_eq!(BaudEmulation::Off, buf.terminal_state.get_baud_emulation());
    update_buffer(&mut buf, &mut caret, &mut parser, b"\x1B[0;8*r");
    assert_eq!(BaudEmulation::Rate(38400), buf.terminal_state.get_baud_emulation());
}

#[test]
fn test_rect_checksum_decrqcra() {
    let (mut buf, mut caret) = create_buffer(b"");
    for _ in 0..20 {
        update_buffer(&mut buf, &mut caret, &mut parser, b"aaaaaaaaaaaaaaaaaaaaaa\n\r");
    }

    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[42;1;1;1;10;10*y");
    assert_eq!(CallbackAction::SendString("\u{1b}P42!~25E4\u{1b}\\".to_string()), act);
}

#[test]
fn test_macro_space_report() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[?62n");
    assert_eq!(CallbackAction::SendString("\x1B[32767*{".to_string()), act);
}


#[test]
fn test_request_tab_stop_report() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act = get_action(&mut buf, &mut caret, &mut parser, b"#\x1B[2$w");
    assert_eq!(CallbackAction::SendString("\x1BP2$u1/9/17/25/33/41/49/57/65/73\x1B\\".to_string()), act);
}

#[test]
fn test_clear_all_tab_stops() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act: CallbackAction = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[3g\x1B[2$w");
    assert_eq!(CallbackAction::SendString("\x1BP2$u\x1B\\".to_string()), act);
}

#[test]
fn test_clear_tab_at_pos() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[16C\x1B[g\x1B[2$w");
    assert_eq!(CallbackAction::SendString("\x1BP2$u1/9/25/33/41/49/57/65/73\x1B\\".to_string()), act);
}

#[test]
fn test_delete_tab() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[41 d\x1B[49 d\x1B[17 d\x1B[2$w");
    assert_eq!(CallbackAction::SendString("\x1BP2$u1/9/25/33/57/65/73\x1B\\".to_string()), act);
}

#[test]
fn set_tab() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act: CallbackAction = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[3g\x1B[1;60H\x1BH\x1B[2$w");
    assert_eq!(CallbackAction::SendString("\x1BP2$u60\x1B\\".to_string()), act);
}

#[test]
fn test_aps_mode_report() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[=1n");
    assert_eq!(CallbackAction::SendString("\x1B[=1;0n".to_string()), act);
    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[=2n");
    assert_eq!(CallbackAction::SendString("\x1B[=2;1n".to_string()), act);
    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[=3n");
    assert_eq!(CallbackAction::SendString("\x1B[=3;1n".to_string()), act);
}

#[test]
fn test_window_manipulation() {
    let (mut buf, mut caret) = create_buffer(b"");
    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[8;25;80t");
    assert_eq!(CallbackAction::ResizeTerminal(80, 25), act);
}

#[test]
fn test_cterm_device_attributes() {
    let (mut buf, mut caret) = create_buffer(b"");

    let act = get_action(&mut buf, &mut caret, &mut parser, b"\x1B[<0c");
    assert_eq!(CallbackAction::SendString("\x1B[<1;2;3;4;5;6;7c".to_string()), act);
}
*/
