//! Comprehensive ANSI Music parser tests
//!
//! Tests all MusicOption modes and various music command sequences

use icy_parser_core::{AnsiMusic, AnsiParser, CommandParser, CommandSink, MusicAction, MusicOption, TerminalCommand};

/// Test sink that captures emitted commands
struct TestSink {
    commands: Vec<TerminalCommand>,
}

impl TestSink {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }

    fn get_music(&self) -> Option<&AnsiMusic> {
        for cmd in &self.commands {
            if let TerminalCommand::AnsiMusic(music) = cmd {
                return Some(music);
            }
        }
        None
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, _text: &[u8]) {}
    fn emit(&mut self, cmd: TerminalCommand) {
        self.commands.push(cmd);
    }
    fn emit_rip(&mut self, _cmd: icy_parser_core::RipCommand) {}
    fn emit_skypix(&mut self, _cmd: icy_parser_core::SkypixCommand) {}
    fn emit_igs(&mut self, _cmd: icy_parser_core::IgsCommand) {}
    fn device_control(&mut self, _dcs: icy_parser_core::DeviceControlString) {}
    fn operating_system_command(&mut self, _osc: icy_parser_core::OperatingSystemCommand) {}
    fn aps(&mut self, _data: &[u8]) {}
    fn play_music(&mut self, music: AnsiMusic) {
        // Convert to TerminalCommand for easier testing
        self.commands.push(TerminalCommand::AnsiMusic(music));
    }
    fn report_error(&mut self, _error: icy_parser_core::ParseError) {}
}

#[test]
fn test_music_option_off() {
    // When MusicOption::Off, CSI M should be Delete Line
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Off);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[MC\x0E", &mut sink);

    // Should emit Delete Line, not music
    assert!(sink.get_music().is_none());
    assert!(sink.commands.iter().any(|cmd| matches!(cmd, TerminalCommand::CsiDeleteLine(_))));
}

#[test]
fn test_music_option_conflicting_with_csi_m() {
    // When MusicOption::Conflicting, CSI M triggers music
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Conflicting);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[MC\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music command");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, len, dotted) = music.music_actions[0] {
        assert!((freq - 523.2511).abs() < 0.01); // C5 (middle C)
        assert_eq!(4 * 120, len); // Quarter note at default tempo 120
        assert!(!dotted);
    } else {
        panic!("Expected PlayNote action");
    }
}

#[test]
fn test_music_option_conflicting_with_csi_n() {
    // When MusicOption::Conflicting, CSI N should NOT trigger music (only Banana style)
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Conflicting);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[NC\x0E", &mut sink);

    // CSI N with Conflicting should not produce music
    assert!(sink.get_music().is_none());
}

#[test]
fn test_music_option_banana_with_csi_n() {
    // When MusicOption::Banana, CSI N triggers music
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Banana);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[NC\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music command");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, _, _) = music.music_actions[0] {
        assert!((freq - 523.2511).abs() < 0.01); // C5
    } else {
        panic!("Expected PlayNote action");
    }
}

#[test]
fn test_music_option_banana_with_csi_m() {
    // When MusicOption::Banana, CSI M should NOT trigger music
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Banana);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[MC\x0E", &mut sink);

    // CSI M with Banana should not produce music
    assert!(sink.get_music().is_none());
    assert!(sink.commands.iter().any(|cmd| matches!(cmd, TerminalCommand::CsiDeleteLine(_))));
}

#[test]
fn test_music_option_both_with_csi_m() {
    // When MusicOption::Both, both CSI M and N trigger music
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[MC\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music with Both + M");
    assert_eq!(1, music.music_actions.len());
}

#[test]
fn test_music_option_both_with_csi_n() {
    // When MusicOption::Both, both CSI M and N trigger music
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[NC\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music with Both + N");
    assert_eq!(1, music.music_actions.len());
}

#[test]
fn test_single_note() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[NC\x0E", &mut sink);

    eprintln!("Commands received: {}", sink.commands.len());
    for (i, cmd) in sink.commands.iter().enumerate() {
        eprintln!("  Command {}: {:?}", i, std::mem::discriminant(cmd));
    }

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, len, dotted) = music.music_actions[0] {
        assert!((freq - 523.2511).abs() < 0.01); // C5
        assert_eq!(4 * 120, len); // Default: quarter note, tempo 120
        assert!(!dotted);
    } else {
        panic!("Expected PlayNote");
    }
}

#[test]
fn test_set_length() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Set length to 8 (eighth note), then play C
    parser.parse(b"\x1B[NL8C\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, len, _) = music.music_actions[0] {
        assert!((freq - 523.2511).abs() < 0.01); // C5
        assert_eq!(8 * 120, len); // Eighth note at tempo 120
    } else {
        panic!("Expected PlayNote");
    }
}

#[test]
fn test_set_octave() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Set octave to 4, play C (should be C4)
    parser.parse(b"\x1B[NO4C\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, _, _) = music.music_actions[0] {
        assert!((freq - 261.6256).abs() < 0.01); // C4 (one octave below C5)
    } else {
        panic!("Expected PlayNote");
    }
}

#[test]
fn test_set_tempo() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Set tempo to 200, play C
    parser.parse(b"\x1B[NT200C\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, len, _) = music.music_actions[0] {
        assert!((freq - 523.2511).abs() < 0.01); // C5
        assert_eq!(4 * 200, len); // Quarter note at tempo 200
    } else {
        panic!("Expected PlayNote");
    }
}

#[test]
fn test_pause() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Pause for 32nd note (dotted)
    parser.parse(b"\x1B[NP32.\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::Pause(len) = music.music_actions[0] {
        assert_eq!(32 * 120 * 3 / 2, len); // Dotted 32nd note: 32 * 120 * 1.5
    } else {
        panic!("Expected Pause, got {:?}", music.music_actions[0]);
    }
}

#[test]
fn test_dotted_note() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Play dotted C
    parser.parse(b"\x1B[NC.\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(_, _, dotted) = music.music_actions[0] {
        assert!(dotted, "Note should be dotted");
    } else {
        panic!("Expected PlayNote");
    }
}

#[test]
fn test_all_notes_in_octave() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Play all notes: C C# D D# E F F# G G# A A# B
    parser.parse(b"\x1B[NCDC+DED-EFE+FGF+GAG+ABA+B\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    // Should have 12 notes (some with sharps/flats)
    assert!(music.music_actions.len() >= 12, "Should have at least 12 notes");
}

#[test]
fn test_melody_sequence() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Complex melody from original tests: T225 O3 L8 G G G L2 E- P8 L8 F F F L2 D
    // Should produce: 3 G notes + 1 E-flat + 1 Pause + 3 F notes + 1 D = 9 actions
    parser.parse(b"\x1B[MT225O3L8GL8GL8GL2E-P8L8FL8FL8FL2D\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(9, music.music_actions.len(), "Should have 9 actions in melody (3G + 1E + 1P + 3F + 1D)");
}

#[test]
fn test_music_styles() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Test different music styles: MF (foreground), MB (background), MN (normal), ML (legato), MS (staccato)
    parser.parse(b"\x1B[NMFC\x0E", &mut sink);
    let music = sink.get_music().expect("Should have music");
    assert!(music.music_actions.len() >= 1);
}

#[test]
fn test_play_note_by_number() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Play note number 49 (A4 = 440Hz, the reference pitch)
    parser.parse(b"\x1B[NN49\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, _, _) = music.music_actions[0] {
        assert!((freq - 440.0).abs() < 0.01, "Note 49 should be A4 (440Hz), got {}", freq);
    } else {
        panic!("Expected PlayNote");
    }
}

#[test]
fn test_terminator() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Music must end with 0x0E (Shift Out)
    parser.parse(b"\x1B[NC", &mut sink);
    assert!(sink.get_music().is_none(), "Music without terminator should not emit");

    let mut sink2 = TestSink::new();
    parser.parse(b"D\x0E", &mut sink2);
    assert!(sink2.get_music().is_some(), "Music with terminator should emit");
}

#[test]
fn test_octave_up_down() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Test octave up (>) and down (<)
    parser.parse(b"\x1B[NO3C>C<C\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(3, music.music_actions.len());

    // First C at octave 3
    if let MusicAction::PlayNote(freq1, _, _) = music.music_actions[0] {
        // Second C at octave 4 (octave up)
        if let MusicAction::PlayNote(freq2, _, _) = music.music_actions[1] {
            // Third C back at octave 3 (octave down)
            if let MusicAction::PlayNote(freq3, _, _) = music.music_actions[2] {
                assert!((freq2 / freq1 - 2.0).abs() < 0.01, "Octave up should double frequency");
                assert!((freq1 - freq3).abs() < 0.01, "Should return to original octave");
            } else {
                panic!("Expected third PlayNote");
            }
        } else {
            panic!("Expected second PlayNote");
        }
    } else {
        panic!("Expected first PlayNote");
    }
}
