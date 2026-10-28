//! Comprehensive ANSI Music parser tests
//!
//! Tests all MusicOption modes and various music command sequences

use icy_parser_core::{AnsiMusic, AnsiParser, CommandParser, CommandSink, MusicAction, MusicOption, TerminalCommand};

/// Test sink that captures emitted commands
struct TestSink {
    commands: Vec<TerminalCommand>,
    music: Option<AnsiMusic>,
    printed: Vec<u8>,
}

impl TestSink {
    fn new() -> Self {
        Self {
            commands: Vec::new(),
            music: None,
            printed: Vec::new(),
        }
    }

    fn get_music(&self) -> Option<&AnsiMusic> {
        self.music.as_ref()
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, text: &[u8]) {
        self.printed.extend_from_slice(text);
    }
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
        self.music = Some(music);
    }
    fn report_error(&mut self, _error: icy_parser_core::ParseError, _level: icy_parser_core::ErrorLevel) {}
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
        assert!((freq - 1046.5023).abs() < 0.01); // O4 C (CTerm manual)
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
        assert!((freq - 1046.5023).abs() < 0.01); // O4 C (CTerm manual)
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
fn test_music_allows_spaces_between_phrases() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[MMBO2G8BBA8G8F8E8D8C2 A16G16E16G8D2 A8O3C8O2G8E8C8\x0E", &mut sink);

    let music = sink.get_music().expect("spaced ANSI music should produce music");
    assert!(!music.music_actions.is_empty());
    assert!(sink.printed.is_empty(), "music text leaked into terminal output: {:?}", sink.printed);
}

#[test]
fn test_single_note() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[NC\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, len, dotted) = music.music_actions[0] {
        assert!((freq - 1046.5023).abs() < 0.01); // O4 C (CTerm manual)
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
        assert!((freq - 1046.5023).abs() < 0.01); // O4 C (CTerm manual)
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

    // Set octave to 3, play C (one octave below the default O4 C)
    parser.parse(b"\x1B[NO3C\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, _, _) = music.music_actions[0] {
        assert!((freq - 523.2511).abs() < 0.01); // O3 C (CTerm manual)
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
        assert!((freq - 1046.5023).abs() < 0.01); // O4 C (CTerm manual)
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

    if let MusicAction::Pause(len, dotted) = music.music_actions[0] {
        // Dotted 32nd note: 1.5 times 240000 / (32 * 120) ms
        assert_eq!(MusicAction::Pause(len, dotted).get_duration(), 360000 / (32 * 120));
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
    assert!(!music.music_actions.is_empty());
}

#[test]
fn test_play_note_by_number() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Note 0 is C in octave 0, so note 33 is A in octave 2 (440 Hz)
    parser.parse(b"\x1B[NN33\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, _, _) = music.music_actions[0] {
        assert!((freq - 440.0).abs() < 0.01, "Note 33 should be 440 Hz, got {}", freq);
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

#[test]
fn test_pipe_music_command() {
    // CSI | should trigger music when music is not Off
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Conflicting);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[|C\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music with | command");
    assert_eq!(1, music.music_actions.len());

    if let MusicAction::PlayNote(freq, _, _) = music.music_actions[0] {
        assert!((freq - 1046.5023).abs() < 0.01); // O4 C (CTerm manual)
    } else {
        panic!("Expected PlayNote");
    }
}

#[test]
fn test_pipe_music_command_off() {
    // When MusicOption::Off, CSI | should not trigger music
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Off);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[|C\x0E", &mut sink);

    // Should not emit music when Off
    assert!(sink.get_music().is_none());
}

#[test]
fn test_pipe_music_with_banana() {
    // CSI | should work with Banana mode
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Banana);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[|T180L4CDEFGAB\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music with | in Banana mode");
    assert_eq!(7, music.music_actions.len(), "Should have 7 notes (C D E F G A B)");
}

#[test]
fn test_pipe_music_with_both() {
    // CSI | should work with Both mode
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[|O4L8CDEFG\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music with | in Both mode");
    assert_eq!(5, music.music_actions.len(), "Should have 5 notes");
}

#[test]
fn test_pipe_vs_m_vs_n_commands() {
    // Compare all three music triggers in Both mode
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);

    // Test CSI M
    let mut sink_m = TestSink::new();
    parser.parse(b"\x1B[MC\x0E", &mut sink_m);
    assert!(sink_m.get_music().is_some(), "CSI M should work");

    // Test CSI N
    let mut sink_n = TestSink::new();
    parser.parse(b"\x1B[ND\x0E", &mut sink_n);
    assert!(sink_n.get_music().is_some(), "CSI N should work");

    // Test CSI |
    let mut sink_pipe = TestSink::new();
    parser.parse(b"\x1B[|E\x0E", &mut sink_pipe);
    assert!(sink_pipe.get_music().is_some(), "CSI | should work");
}

#[test]
fn test_pipe_complex_sequence() {
    // Test a complex music sequence with CSI |
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // Complex melody: set tempo, octave, length, play notes with pauses
    parser.parse(b"\x1B[|T160O4L8CDEP4L16FGL4A\x0E", &mut sink);

    let music = sink.get_music().expect("Should have music");
    // Should have: C, D, E, Pause, F, G, A = 7 actions
    assert_eq!(7, music.music_actions.len(), "Should have 7 actions (3 notes + pause + 3 notes)");

    // Verify pause is in the sequence
    let has_pause = music.music_actions.iter().any(|action| matches!(action, MusicAction::Pause(..)));
    assert!(has_pause, "Should contain a pause action");
}

#[test]
fn test_mso1d16_parsed_as_music() {
    // ESC[MSO1D16\x0E — M-style music parses one leading style character
    // before entering the command loop.
    // S = staccato, O1 = octave 1, D16 = note D length 16
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Conflicting);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[MSO1D16\x0E", &mut sink);

    let music = sink.get_music().expect("ESC[MSO1D16 should produce music");
    assert_eq!(2, music.music_actions.len(), "Should have 2 actions: SetStyle + PlayNote");

    // First action: staccato style (from leading 'S')
    assert!(
        matches!(music.music_actions[0], MusicAction::SetStyle(icy_parser_core::MusicStyle::Staccato)),
        "First action should be SetStyle(Staccato), got {:?}",
        music.music_actions[0]
    );

    // Second action: note D in octave 1 with length 16
    if let MusicAction::PlayNote(freq, len, dotted) = music.music_actions[1] {
        // O1 D (CTerm manual): ~146.83 Hz
        assert!((freq - 146.8324).abs() < 0.1, "Expected O1 D (~146.8 Hz), got {}", freq);
        assert_eq!(16 * 120, len, "Length should be 16 * tempo(120)");
        assert!(!dotted);
    } else {
        panic!("Expected PlayNote, got {:?}", music.music_actions[1]);
    }
}

#[test]
fn test_bansi_skips_leading_style_char() {
    // ESC[N (BANSI style) does NOT parse a leading style character —
    // it jumps straight into the command loop, where 'S' does nothing.
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Banana);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[NSO1D16\x0E", &mut sink);

    let music = sink.get_music().expect("ESC[NSO1D16 should produce music");
    assert!(
        matches!(music.music_actions[..], [MusicAction::PlayNote(_, 1920, false)]),
        "BANSI should NOT parse leading style char, got {:?}",
        music.music_actions
    );
}

#[test]
fn test_bansi_direct_commands() {
    // ESC[N (BANSI style) goes directly into command loop.
    // ESC[NO1D16 = octave 1, note D length 16 (no style prefix)
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Banana);
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[NO1D16\x0E", &mut sink);

    let music = sink.get_music().expect("ESC[NO1D16 should produce music");
    assert_eq!(1, music.music_actions.len(), "Should have 1 action: PlayNote");

    if let MusicAction::PlayNote(freq, len, dotted) = music.music_actions[0] {
        assert!((freq - 146.8324).abs() < 0.1, "Expected O1 D (~146.8 Hz), got {}", freq);
        assert_eq!(16 * 120, len, "Length should be 16 * tempo(120)");
        assert!(!dotted);
    } else {
        panic!("Expected PlayNote, got {:?}", music.music_actions[0]);
    }
}

fn note_frequencies(music: &AnsiMusic) -> Vec<f32> {
    music
        .music_actions
        .iter()
        .filter_map(|action| match action {
            MusicAction::PlayNote(freq, _, _) => Some(*freq),
            _ => None,
        })
        .collect()
}

#[test]
fn test_octaves_follow_the_cterm_manual() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::SyncTerm);
    let mut sink = TestSink::new();

    // Octaves 0-2 are distinct, and O4 is two octaves above O2.
    parser.parse(b"\x1B[|O0CO1CO2CO4C\x0E", &mut sink);

    let freqs = note_frequencies(sink.get_music().unwrap());
    let expected = [65.4064, 130.8128, 261.6256, 1046.5023];
    assert_eq!(freqs.len(), expected.len());
    for (freq, expected) in freqs.iter().zip(expected) {
        assert!((freq - expected).abs() < 0.01, "got {freqs:?}");
    }
}

#[test]
fn test_music_state_carries_over_between_strings() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::SyncTerm);
    let mut sink = TestSink::new();

    // Octave, tempo, length and articulation persist, as in CTerm.
    parser.parse(b"\x1B[|O2T200L8MSC\x0E", &mut sink);
    parser.parse(b"\x1B[|C\x0E", &mut sink);
    let music = sink.get_music().unwrap();
    assert_eq!(music.music_actions[0], MusicAction::SetStyle(icy_parser_core::MusicStyle::Staccato));
    assert!(matches!(music.music_actions[1], MusicAction::PlayNote(_, 1600, false)));
    assert!((note_frequencies(music)[0] - 261.6256).abs() < 0.01);

    // RIS resets them.
    parser.parse(b"\x1Bc\x1B[|C\x0E", &mut sink);
    let music = sink.get_music().unwrap();
    assert!(matches!(music.music_actions[..], [MusicAction::PlayNote(_, 480, false)]));
    assert!((note_frequencies(music)[0] - 1046.5023).abs() < 0.01);
}

#[test]
fn test_lower_case_and_dotted_length() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::SyncTerm);
    let mut sink = TestSink::new();

    // A dot after an explicit length makes the note 1.5 times as long.
    parser.parse(b"\x1B[|t120c8.\x0E", &mut sink);

    let music = sink.get_music().unwrap();
    assert_eq!(music.music_actions.len(), 1);
    assert_eq!(music.music_actions[0].get_duration(), 360000 / (120 * 8));
}

#[test]
fn test_csi_equals_m_selects_music_introducers() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // CSI = 0 M: only CSI | starts music; CSI M is Delete Line again.
    parser.parse(b"\x1B[=0M\x1B[MC\x0E", &mut sink);
    assert!(sink.get_music().is_none());
    assert!(sink.commands.iter().any(|cmd| matches!(cmd, TerminalCommand::CsiDeleteLine(1))));
    assert_eq!(parser.music_option(), MusicOption::SyncTerm);

    // CSI = 1 M: CSI N starts music too.
    parser.parse(b"\x1B[=1M\x1B[NC\x0E", &mut sink);
    assert!(sink.get_music().is_some());
    assert_eq!(parser.music_option(), MusicOption::Banana);

    // CSI = 2 M: CSI M as well.
    parser.parse(b"\x1B[=2M", &mut sink);
    assert_eq!(parser.music_option(), MusicOption::Both);

    // Music the embedder turned off stays off.
    parser.set_music_option(MusicOption::Off);
    parser.parse(b"\x1B[=2M", &mut sink);
    assert_eq!(parser.music_option(), MusicOption::Off);
}

#[test]
fn test_ris_restores_the_configured_music_option() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    let mut sink = TestSink::new();

    // RIS undoes CSI = Ps M and returns to the option the embedder set.
    parser.parse(b"\x1B[=0M\x1Bc", &mut sink);
    assert_eq!(parser.music_option(), MusicOption::Both);

    parser.set_music_option(MusicOption::Banana);
    parser.parse(b"\x1B[=2M\x1Bc\x1B[MC\x0E", &mut sink);
    assert_eq!(parser.music_option(), MusicOption::Banana);
    assert!(sink.get_music().is_none());
    assert!(sink.commands.iter().any(|cmd| matches!(cmd, TerminalCommand::CsiDeleteLine(1))));
}

#[test]
fn test_music_option_can_change_through_command_parser() {
    let mut parser: Box<dyn CommandParser> = Box::new(AnsiParser::new());
    let mut sink = TestSink::new();

    parser.set_music_option(MusicOption::SyncTerm);
    parser.parse(b"\x1B[|C\x0E", &mut sink);

    assert!(sink.get_music().is_some());
}

fn durations(parser: &mut AnsiParser, string: &str) -> Vec<i32> {
    let mut sink = TestSink::new();
    parser.parse(format!("\x1B[|{string}\x0E").as_bytes(), &mut sink);
    sink.get_music().unwrap().music_actions.iter().map(MusicAction::get_duration).collect()
}

#[test]
fn test_dotted_pause_lasts_one_and_a_half_times_as_long() {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::SyncTerm);

    // At tempos like these, 240000 / (tempo * len * 2 / 3) rounds the wrong way.
    assert_eq!(durations(&mut parser, "T97P1.P4.P8."), vec![3711, 927, 463]);
    assert_eq!(durations(&mut parser, "T121P1.P4.P16."), vec![2975, 743, 185]);

    // A dotted pause lasts exactly as long as a dotted note, and an undotted one
    // as long as an undotted note, at every tempo and length.
    for tempo in 32..=255 {
        for len in 1..=64 {
            let (plain, dotted) = (240000 / (tempo * len), 360000 / (tempo * len));
            let d = durations(&mut parser, &format!("T{tempo}L{len}PCP.C."));
            assert_eq!(d, vec![plain, plain, dotted, dotted], "T{tempo} L{len}");
        }
    }
}
