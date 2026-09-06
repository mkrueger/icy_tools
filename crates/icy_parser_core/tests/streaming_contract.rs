//! Chunk boundaries are transport details, not end-of-stream markers.
//! Compare syntax events, not screen execution (notably for Viewdata and IGS loops).

use icy_parser_core::{
    AnsiMusic, AnsiParser, AsciiParser, AtasciiParser, AvatarParser, CommandParser, CommandSink, CtrlAParser, DeviceControlString, ErrorLevel, IgsCommand,
    IgsParser, Mode7Parser, MusicOption, OperatingSystemCommand, ParseError, PcBoardParser, PetsciiParser, RenegadeParser, RipCommand, RipParser,
    SkypixCommand, SkypixParser, TerminalCommand, TerminalRequest, VT52Mode, ViewDataCommand, ViewdataParser, Vt52Parser,
};

/// Keep every callback in one ordered stream. Never decode individual print chunks:
/// a chunk may end in the middle of a Unicode codepoint or contain legacy bytes.
#[derive(Debug, Clone, PartialEq)]
enum Event {
    Print(Vec<u8>),
    Emit(TerminalCommand),
    Rip(RipCommand),
    Skypix(SkypixCommand),
    Igs(IgsCommand),
    Viewdata(ViewDataCommand),
    Osc(OperatingSystemCommand),
    Dcs(DeviceControlString),
    Aps(Vec<u8>),
    Music(AnsiMusic),
    Request(TerminalRequest),
    Error(ParseError, ErrorLevel),
    BeginIgsXor,
    EndIgsXor,
}

#[derive(Default)]
struct Recorder {
    events: Vec<Event>,
}

impl CommandSink for Recorder {
    fn print(&mut self, text: &[u8]) {
        if text.is_empty() {
            return;
        }
        if let Some(Event::Print(bytes)) = self.events.last_mut() {
            bytes.extend_from_slice(text);
        } else {
            self.events.push(Event::Print(text.to_vec()));
        }
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.events.push(Event::Emit(cmd));
    }

    fn emit_rip(&mut self, cmd: RipCommand) {
        self.events.push(Event::Rip(cmd));
    }

    fn emit_skypix(&mut self, cmd: SkypixCommand) {
        self.events.push(Event::Skypix(cmd));
    }

    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.events.push(Event::Igs(cmd));
    }

    fn emit_view_data(&mut self, cmd: ViewDataCommand) {
        self.events.push(Event::Viewdata(cmd));
    }

    fn device_control(&mut self, dcs: DeviceControlString) {
        self.events.push(Event::Dcs(dcs));
    }

    fn operating_system_command(&mut self, osc: OperatingSystemCommand) {
        self.events.push(Event::Osc(osc));
    }

    fn aps(&mut self, data: &[u8]) {
        self.events.push(Event::Aps(data.to_vec()));
    }

    fn play_music(&mut self, music: AnsiMusic) {
        self.events.push(Event::Music(music));
    }

    fn request(&mut self, request: TerminalRequest) {
        self.events.push(Event::Request(request));
    }

    fn report_error(&mut self, error: ParseError, level: ErrorLevel) {
        self.events.push(Event::Error(error, level));
    }

    fn begin_igs_xor_mode(&mut self) {
        self.events.push(Event::BeginIgsXor);
    }

    fn end_igs_xor_mode(&mut self) {
        self.events.push(Event::EndIgsXor);
    }
}

#[derive(Debug, Clone, Copy)]
enum ParserKind {
    Ascii,
    Ansi,
    Avatar,
    Pcboard,
    Ctrla,
    Renegade,
    Atascii,
    Petscii,
    Viewdata,
    Mode7,
    Rip,
    Vt52,
    Igs,
    Skypix,
}

impl ParserKind {
    fn create(self) -> Box<dyn CommandParser> {
        match self {
            Self::Ascii => Box::new(AsciiParser::new()),
            Self::Ansi => {
                let mut parser = AnsiParser::new();
                parser.set_music_option(MusicOption::Both);
                Box::new(parser)
            }
            Self::Avatar => Box::new(AvatarParser::new()),
            Self::Pcboard => Box::new(PcBoardParser::new()),
            Self::Ctrla => Box::new(CtrlAParser::new()),
            Self::Renegade => Box::new(RenegadeParser::new()),
            Self::Atascii => Box::new(AtasciiParser::new()),
            Self::Petscii => Box::new(PetsciiParser::new()),
            Self::Viewdata => Box::new(ViewdataParser::new()),
            Self::Mode7 => Box::new(Mode7Parser::new()),
            Self::Rip => Box::new(RipParser::new()),
            Self::Vt52 => Box::new(Vt52Parser::new(VT52Mode::Mixed)),
            Self::Igs => Box::new(IgsParser::new()),
            Self::Skypix => Box::new(SkypixParser::new()),
        }
    }
}

fn whole(kind: ParserKind, input: &[u8]) -> Vec<Event> {
    let mut sink = Recorder::default();
    kind.create().parse(input, &mut sink);
    sink.events
}

/// `ends` lists cumulative chunk ends, including duplicates for empty chunks.
/// Empty calls are also inserted at every boundary and checked immediately, so
/// an incomplete command cannot be silently finalized and later compensated for.
fn partitioned(kind: ParserKind, input: &[u8], ends: &[usize]) -> Vec<Event> {
    let mut parser = kind.create();
    let mut sink = Recorder::default();
    let mut start = 0;
    for &end in ends {
        assert!(start <= end && end <= input.len());
        let before_empty = sink.events.clone();
        parser.parse(b"", &mut sink);
        parser.parse(b"", &mut sink);
        assert_eq!(
            sink.events, before_empty,
            "{kind:?}: empty input emitted events at {start}; input={input:02x?}, ends={ends:?}"
        );
        parser.parse(&input[start..end], &mut sink);
        start = end;
    }
    assert_eq!(start, input.len());
    let before_empty = sink.events.clone();
    parser.parse(b"", &mut sink);
    parser.parse(b"", &mut sink);
    assert_eq!(
        sink.events, before_empty,
        "{kind:?}: empty input finalized stream; input={input:02x?}, ends={ends:?}"
    );
    sink.events
}

fn check_contract(kind: ParserKind, name: &str, input: &[u8]) -> Vec<Event> {
    let expected = whole(kind, input);
    assert_eq!(whole(kind, input), expected, "{kind:?}/{name}: non-deterministic whole parse");

    // Includes [empty, whole], [whole, empty], and [empty, empty].
    for split in 0..=input.len() {
        let ends = [split, input.len()];
        assert_eq!(
            partitioned(kind, input, &ends),
            expected,
            "{kind:?}/{name}: two-way split={split}; input={input:02x?}"
        );
    }

    let bytewise: Vec<_> = (0..=input.len()).collect();
    assert_eq!(partitioned(kind, input, &bytewise), expected, "{kind:?}/{name}: bytewise; input={input:02x?}");

    // Local, fixed xorshift64: reproducible across runs, platforms, and test order.
    // Repeated sorted cuts exercise empty chunks without risking a zero-progress loop.
    for seed in 1..=32_u64 {
        let mut state = 0x4d59_5df4_d0f3_3173 ^ seed;
        let mut ends = vec![0, input.len(), input.len()];
        for _ in 0..(2 + seed % 13) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ends.push((state % (input.len() as u64 + 1)) as usize);
        }
        ends.sort_unstable();
        assert_eq!(
            partitioned(kind, input, &ends),
            expected,
            "{kind:?}/{name}: random seed={seed}, ends={ends:?}; input={input:02x?}"
        );
    }
    expected
}

macro_rules! parser_contract {
    ($test:ident, $kind:ident, $($name:literal => $input:expr),+ $(,)?) => {
        #[test]
        fn $test() {
            let kind = ParserKind::$kind;
            assert!(check_contract(kind, "empty", b"").is_empty());
            $(
                let events = check_contract(kind, $name, $input);
                assert!(
                    !events.iter().any(|event| matches!(event, Event::Error(..))),
                    "{kind:?}/{}: valid or incomplete sample reported an error: {events:?}", $name
                );
            )+
        }
    };
}

// Samples are small combinations of the existing format-specific integration
// tests. Explicit incomplete samples deliberately have no synthetic EOF/flush.
parser_contract!(ascii, Ascii,
    "controls (ascii_utf8.rs)" => b"A\r\nB\tC\x08D\x0cE\x07F\x7fG",
    "unicode bytes" => "Aäα😀Z".as_bytes(),
    "incomplete unicode bytes" => b"A\xf0\x9f",
);
parser_contract!(ansi, Ansi,
    "CSI and requests (ansi/cursor.rs, requests.rs)" => b"A\x1b[2;3HB\x1b[1;31mC\x1b[0m\x1b[6n\x1b[cZ",
    "music (ansi/music.rs)" => b"A\x1b[MT120O4L8CDE\x0eZ",
    "incomplete ESC" => b"A\x1b",
    "incomplete CSI" => b"A\x1b[38;2;1;",
    "incomplete music" => b"A\x1b[MT120O",
);
parser_contract!(avatar, Avatar,
    "color, cursor, repeat (avatar_parser.rs)" => b"A\x16\x01\x07B\x16\x03\x19x\x03C\x1b[31mD\x1b[0m",
    "incomplete color" => b"A\x16\x01",
    "incomplete repeat" => b"A\x19x",
    "incomplete embedded ANSI" => b"A\x1b[31;",
);
parser_contract!(pcboard, Pcboard,
    "colors and literal at (pcboard_parser.rs)" => b"A@X0FB@@C@X1ED\x1b[2;3HE",
    "incomplete at" => b"A@",
    "incomplete color" => b"A@X0",
    "incomplete embedded ANSI" => b"A\x1b[31;",
);
parser_contract!(ctrla, Ctrla,
    "colors, bold, literal (ctrla_parser.rs)" => b"A\x01RRed\x01H\x01WWhite\x01N\x01AZ\x1b[2;3H!",
    "incomplete ctrl-a" => b"A\x01",
    "incomplete embedded ANSI" => b"A\x1b[31;",
);
parser_contract!(renegade, Renegade,
    "colors and literal pipe (renegade_parser.rs)" => b"A|04Red|17Blue|15White|Hello\x1b[2;3H!",
    "incomplete pipe" => b"A|",
    "incomplete color" => b"A|1",
    "incomplete embedded ANSI" => b"A\x1b[31;",
);
parser_contract!(atascii, Atascii,
    "controls and escaped control (atascii_parser.rs)" => b"A\x1cB\x1dC\x1eD\x1fE\x9bF\x1b\x1cG\x7dH",
    "incomplete escape" => b"A\x1b",
);
parser_contract!(petscii, Petscii,
    "color, movement, charset (petscii/mod.rs)" => b"HI\x11\x1cRED\x13\x0eTest\x0fTEST\x1bQ\x0dZ",
    "incomplete escape" => b"A\x1b",
);
parser_contract!(viewdata, Viewdata,
    "graphics, hold, rows (viewdata/mod.rs)" => b"HI\x1bQ!\x1b^\x1bZ?\x1b_\x0a\x1bMDouble\x1bL\x1eZ",
    "C1 escaped control" => b"A\x9bQ!\x8aZ",
    "incomplete escape" => b"A\x1b",
);
parser_contract!(mode7, Mode7,
    "VDU, graphics, height (mode7_parser.rs)" => b"HI\x81RED\x91!\x9e\x9a?\x9f\x8dDouble\x8c\x0d\x1f\x03\x02Z",
    "incomplete VDU position" => b"A\x1f\x03",
);
parser_contract!(rip, Rip,
    "viewport and text (rip/mod.rs)" => b"!|v00000M09|c05|THello\n\x1b[31mANSI\x1b[0m",
    "incomplete introducer" => b"!|",
    "incomplete coordinates" => b"!|v0000",
    "incomplete text" => b"!|THello",
    "incomplete embedded ANSI" => b"A\x1b[31;",
);
parser_contract!(vt52, Vt52,
    "position, color, save/restore (vt52_parser.rs)" => b"A\x1bY%*B\x1bb\x07C\x1bj\x1bH\x1bk\x1bKZ",
    "incomplete position" => b"A\x1bY%",
    "incomplete color" => b"A\x1bb",
);
parser_contract!(igs, Igs,
    "graphics and text (igs/mod.rs)" => b"G#C1,15:B10,20,30,40,0:L0,0,5,5:W10,20,Hi@",
    "embedded VT52" => b"A\x1bY*%B\x1bb\x01C",
    "loop syntax, not execution (igs/loop_run_tests.rs)" => b"G#&>0,2,1,0,L,4,0,0,x,y:",
    "incomplete introducer" => b"G#",
    "incomplete coordinates" => b"G#B10,20,",
    "incomplete text" => b"G#W10,20,Hi",
);
parser_contract!(skypix, Skypix,
    "pixel, line, ANSI (skypix_parser.rs)" => b"A\x1b[1;10;20!\x1b[2;30;40!\x1b[31mRed\x1b[0mZ",
    "incomplete graphics" => b"A\x1b[1;10;",
    "incomplete escape" => b"A\x1b",
);

// Separate tests keep a failing OSC case from masking DCS/APS coverage.
// Every split includes ESC | backslash in ST. See ansi/{osc,dcs,aps}.rs.
macro_rules! string_contract {
    ($test:ident, $input:expr, $event:pat) => {
        #[test]
        fn $test() {
            let input: &[u8] = $input;
            let events = check_contract(ParserKind::Ansi, stringify!($test), input);
            assert!(events.iter().any(|event| matches!(event, $event)), "{events:?}");
            assert!(!events.iter().any(|event| matches!(event, Event::Error(..))), "{events:?}");
            // Proper prefixes include unterminated payloads and a pending ST escape.
            for end in 1..input.len() {
                check_contract(ParserKind::Ansi, stringify!($test), &input[..end]);
            }
        }
    };
}

string_contract!(osc_bel, b"A\x1b]0;Title\x07Z", Event::Osc(_));
string_contract!(osc_st, b"A\x1b]2;Title\x1b\\Z", Event::Osc(_));
string_contract!(dcs_sixel, b"A\x1bP0;0;8q#1!3~-\x1b\\Z", Event::Dcs(_));
string_contract!(dcs_font, b"A\x1bPCTerm:Font:5:dGVzdA==\x1b\\Z", Event::Dcs(_));
string_contract!(aps_st, b"A\x1b_AppCommand\x1b\\Z", Event::Aps(_));
string_contract!(aps_payload_escape, b"A\x1b_Test\x1bData\x1b\\Z", Event::Aps(_));
string_contract!(dcs_request, b"A\x1bP$qm\x1b\\Z", Event::Request(_));
string_contract!(aps_request, b"A\x1b_SyncTERM:Q;JXL\x1b\\Z", Event::Request(_));

macro_rules! embedded_ansi_contract {
    ($test:ident, $kind:ident) => {
        #[test]
        fn $test() {
            check_contract(
                ParserKind::$kind,
                "embedded ANSI strings",
                b"A\x1b]0;Title\x1b\\B\x1bPq~-\x1b\\C\x1b_App\x1b\\Z",
            );
        }
    };
}

embedded_ansi_contract!(ansi_strings, Ansi);
embedded_ansi_contract!(avatar_ansi_strings, Avatar);
embedded_ansi_contract!(pcboard_ansi_strings, Pcboard);
embedded_ansi_contract!(ctrla_ansi_strings, Ctrla);
embedded_ansi_contract!(renegade_ansi_strings, Renegade);
embedded_ansi_contract!(rip_ansi_strings, Rip);

#[test]
fn rip_routing_boundaries_preserve_events() {
    for input in [
        b"plain!inline\r\n!|c05|TTitle\nplain\r\n".as_slice(),
        b"\x1b[1!\r\n!not-rip\n\x1b[2!\r\n!|c05|TTitle\n",
        b"!|THello\\\r\n World\nnext!inline\n",
        b"\x1b[31mRed!\x1b[0m\n!|c05|TTitle\n",
    ] {
        check_contract(ParserKind::Rip, "routing boundaries", input);
    }
}

#[test]
fn rip_preserves_plain_text_batching() {
    #[derive(Default)]
    struct PrintCounter {
        calls: usize,
        bytes: usize,
    }
    impl CommandSink for PrintCounter {
        fn print(&mut self, text: &[u8]) {
            self.calls += 1;
            self.bytes += text.len();
        }
        fn emit(&mut self, _: TerminalCommand) {}
    }
    let input = vec![b'A'; 8192];
    let mut sink = PrintCounter::default();
    RipParser::new().parse(&input, &mut sink);
    assert_eq!(sink.bytes, input.len());
    assert_eq!(sink.calls, 1, "RIP passthrough must not fragment ANSI text runs");
}

#[test]
fn unicode_bytes() {
    for kind in [
        ParserKind::Ansi,
        ParserKind::Avatar,
        ParserKind::Pcboard,
        ParserKind::Ctrla,
        ParserKind::Renegade,
        ParserKind::Rip,
        ParserKind::Skypix,
    ] {
        check_contract(kind, "Unicode split within codepoints", "Aäα😀Z".as_bytes());
    }
    assert_eq!(whole(ParserKind::Ascii, "Aäα😀Z".as_bytes()), vec![Event::Print("Aäα😀Z".as_bytes().to_vec())]);
}

#[test]
fn errors_preserve_payload_level_and_order() {
    // Unknown DCS is intentionally ignored by the current parser. Invalid font
    // base64, in contrast, must report a structured diagnostic between the prints.
    let events = check_contract(ParserKind::Ansi, "invalid font base64", b"A\x1bPCTerm:Font:5:?\x1b\\Z");
    assert_eq!(
        events,
        vec![
            Event::Print(b"A".to_vec()),
            Event::Error(
                ParseError::MalformedSequence {
                    description: "Invalid base64 in DCS font data",
                    sequence: Some("ESC P CTerm:Font:5:...".to_string()),
                    context: Some("Font slot 5".to_string()),
                },
                ErrorLevel::Error,
            ),
            Event::Print(b"Z".to_vec()),
        ]
    );
}

#[test]
fn minimal_osc_split_terminator() {
    // Minimal OSC 0 with empty title: splitting the final ESC from '\\' must
    // still emit SetTitle([]). Keep this failing until the parser is repaired.
    let input = b"\x1b]0;\x1b\\";
    assert_eq!(whole(ParserKind::Ansi, input), vec![Event::Osc(OperatingSystemCommand::SetTitle(vec![]))]);
    check_contract(ParserKind::Ansi, "minimal split ST", input);
}

#[test]
fn minimal_skypix_split_csi() {
    // A single SGR digit must not also be printed when a chunk ends after it.
    check_contract(ParserKind::Skypix, "minimal CSI digit", b"\x1b[1m");
}

#[test]
fn recorder_covers_every_callback_and_only_merges_adjacent_prints() {
    let mut sink = Recorder::default();
    sink.print(b"");
    sink.print(b"A\xf0");
    sink.print(b"\x9f\x98\x80");
    sink.emit(TerminalCommand::Bell);
    sink.print(b"B");
    sink.emit_rip(RipCommand::Home);
    sink.emit_skypix(SkypixCommand::SetPixel { x: 1, y: 2 });
    let igs = IgsCommand::Line {
        x1: 0.into(),
        y1: 0.into(),
        x2: 1.into(),
        y2: 2.into(),
    };
    sink.emit_igs(igs.clone());
    sink.emit_view_data(ViewDataCommand::DoubleHeight(true));
    sink.operating_system_command(OperatingSystemCommand::SetTitle(b"title".to_vec()));
    sink.device_control(DeviceControlString::LoadFont(1, vec![0, 255]));
    let mut borrowed = vec![0, 255];
    sink.aps(&borrowed);
    borrowed.fill(42);
    sink.play_music(AnsiMusic::default());
    sink.request(TerminalRequest::ScreenSizeReport);
    let error = ParseError::UnsupportedFeature { description: "recorder test" };
    // Deliberately different from error.level(): preserve the caller's severity.
    sink.report_error(error.clone(), ErrorLevel::Error);
    sink.begin_igs_xor_mode();
    sink.print(b"C");
    sink.end_igs_xor_mode();
    sink.print(b"D");
    sink.print(b"");
    assert_eq!(
        sink.events,
        vec![
            Event::Print("A😀".as_bytes().to_vec()),
            Event::Emit(TerminalCommand::Bell),
            Event::Print(b"B".to_vec()),
            Event::Rip(RipCommand::Home),
            Event::Skypix(SkypixCommand::SetPixel { x: 1, y: 2 }),
            Event::Igs(igs),
            Event::Viewdata(ViewDataCommand::DoubleHeight(true)),
            Event::Osc(OperatingSystemCommand::SetTitle(b"title".to_vec())),
            Event::Dcs(DeviceControlString::LoadFont(1, vec![0, 255])),
            Event::Aps(vec![0, 255]),
            Event::Music(AnsiMusic::default()),
            Event::Request(TerminalRequest::ScreenSizeReport),
            Event::Error(error, ErrorLevel::Error),
            Event::BeginIgsXor,
            Event::Print(b"C".to_vec()),
            Event::EndIgsXor,
            Event::Print(b"D".to_vec()),
        ]
    );
}
