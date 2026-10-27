//! `CommandParser::parse_until_pause` stops after a blocking construct when the sink asks.

use icy_parser_core::{AnsiMusic, AnsiParser, CommandParser, CommandSink, MusicOption, TerminalCommand};

#[derive(Default)]
struct PauseSink {
    events: Vec<String>,
    pause_on_music: bool,
    wait_apc_seen: bool,
}

impl CommandSink for PauseSink {
    fn print(&mut self, text: &[u8]) {
        self.events.push(String::from_utf8_lossy(text).into_owned());
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.events.push(format!("{cmd:?}"));
    }

    fn aps(&mut self, data: &[u8]) {
        self.wait_apc_seen = data == b"SyncTERM:A;Wait";
        self.events.push(format!("APC {}", String::from_utf8_lossy(data)));
    }

    fn play_music(&mut self, _music: AnsiMusic) {
        self.events.push("music".to_string());
    }

    fn should_pause(&mut self) -> bool {
        let last = self.events.last().map(String::as_str);
        (self.pause_on_music && last == Some("music")) || std::mem::take(&mut self.wait_apc_seen)
    }
}

const INPUT: &[u8] = b"A\x1b[MC\x0eB\x1b_SyncTERM:A;Wait\x1b\\C\x1b_other\x1b\\D";

fn music_parser() -> AnsiParser {
    let mut parser = AnsiParser::new();
    parser.set_music_option(MusicOption::Both);
    parser
}

#[test]
fn parse_until_pause_stops_after_music_and_wait_and_resumes() {
    let mut parser = music_parser();
    let mut sink = PauseSink {
        pause_on_music: true,
        ..Default::default()
    };

    let n = parser.parse_until_pause(INPUT, &mut sink);
    assert_eq!(&INPUT[..n], b"A\x1b[MC\x0e");
    assert_eq!(sink.events, ["A", "music"]);

    let rest = &INPUT[n..];
    let m = parser.parse_until_pause(rest, &mut sink);
    assert_eq!(&rest[..m], b"B\x1b_SyncTERM:A;Wait\x1b\\");
    assert_eq!(sink.events, ["A", "music", "B", "APC SyncTERM:A;Wait"]);

    // Another APC string doesn't pause.
    let rest = &rest[m..];
    assert_eq!(parser.parse_until_pause(rest, &mut sink), rest.len());
    assert_eq!(sink.events, ["A", "music", "B", "APC SyncTERM:A;Wait", "C", "APC other", "D"]);
}

#[test]
fn parse_ignores_pause_requests_and_sinks_that_never_pause_consume_everything() {
    let mut paused = PauseSink {
        pause_on_music: true,
        ..Default::default()
    };
    music_parser().parse(INPUT, &mut paused);

    let mut plain = PauseSink::default();
    let mut parser = music_parser();
    // Without the pause on music, only the Wait APC stops the parse.
    let n = parser.parse_until_pause(INPUT, &mut plain);
    assert_eq!(&INPUT[..n], b"A\x1b[MC\x0eB\x1b_SyncTERM:A;Wait\x1b\\");
    parser.parse_until_pause(&INPUT[n..], &mut plain);

    assert_eq!(paused.events, plain.events);
    assert_eq!(paused.events.len(), 7);
}

/// Pauses after every music string and APC, and records the events like `PauseSink`.
#[derive(Default)]
struct AlwaysPauseSink {
    events: Vec<String>,
}

impl CommandSink for AlwaysPauseSink {
    fn print(&mut self, text: &[u8]) {
        // `print` boundaries may differ between runs; compare the text only.
        match self.events.last_mut() {
            Some(last) if last.starts_with("text:") => last.push_str(&String::from_utf8_lossy(text)),
            _ => self.events.push(format!("text:{}", String::from_utf8_lossy(text))),
        }
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.events.push(format!("{cmd:?}"));
    }

    fn aps(&mut self, data: &[u8]) {
        self.events.push(format!("APC {}", String::from_utf8_lossy(data)));
    }

    fn play_music(&mut self, _music: AnsiMusic) {
        self.events.push("music".to_string());
    }

    fn should_pause(&mut self) -> bool {
        true
    }
}

#[test]
fn resuming_after_every_pause_gives_the_same_events_as_one_parse() {
    let inputs: [&[u8]; 5] = [
        INPUT,
        // back to back, and a construct as the last byte
        b"\x1b[MCDE\x0e\x1b[MFGA\x0e\x1b_one\x1b\\\x1b_two\x1b\\\x1b[MC\x0e",
        // an escape sequence and a split-looking ESC inside the APC string
        b"\x1b[31mX\x1b_a\x1bb\x1b\\\x1b[1;1HY\x1b[MC\x0e\x1b[0m",
        // an unfinished sequence after the pause
        b"\x1b[MC\x0e\x1b[3",
        b"",
    ];
    for input in inputs {
        let mut resumed = AlwaysPauseSink::default();
        let mut parser = music_parser();
        let mut rest = input;
        let mut calls = 0;
        while !rest.is_empty() {
            let n = parser.parse_until_pause(rest, &mut resumed);
            assert!(n > 0 && n <= rest.len(), "no progress on {input:?}");
            rest = &rest[n..];
            calls += 1;
        }
        assert!(calls >= 2 || input.len() < 2, "never paused on {input:?}");
        // The unfinished sequence at the end of one input is completed afterwards.
        parser.parse(b"1m", &mut resumed);
        let mut whole_parser = music_parser();
        let mut whole = AlwaysPauseSink::default();
        whole_parser.parse(input, &mut whole);
        whole_parser.parse(b"1m", &mut whole);

        assert_eq!(resumed.events, whole.events, "{input:?}");
    }
}
