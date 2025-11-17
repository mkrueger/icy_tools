use icy_parser_core::{
    AnsiParser, AtasciiParser, AvatarParser, CommandParser, CommandSink, CtrlAParser, DeviceControlString, IgsCommand, Mode7Parser, OperatingSystemCommand,
    ParseError, PcBoardParser, PetsciiParser, RenegadeParser, RipCommand, SkypixCommand, SkypixParser, TerminalCommand, ViewdataParser,
};

/// A no-op sink that just counts calls - perfect for fuzzing
struct FuzzSink {
    print_count: usize,
    command_count: usize,
    error_count: usize,
}

impl FuzzSink {
    fn new() -> Self {
        Self {
            print_count: 0,
            command_count: 0,
            error_count: 0,
        }
    }
}

impl CommandSink for FuzzSink {
    fn print(&mut self, _text: &[u8]) {
        self.print_count += 1;
    }

    fn emit(&mut self, _cmd: TerminalCommand) {
        self.command_count += 1;
    }

    fn emit_rip(&mut self, _cmd: RipCommand) {
        self.command_count += 1;
    }

    fn emit_skypix(&mut self, _cmd: SkypixCommand) {
        self.command_count += 1;
    }

    fn emit_igs(&mut self, _cmd: IgsCommand) {
        self.command_count += 1;
    }

    fn device_control(&mut self, _dcs: DeviceControlString<'_>) {
        self.command_count += 1;
    }

    fn operating_system_command(&mut self, _osc: OperatingSystemCommand<'_>) {
        self.command_count += 1;
    }

    fn aps(&mut self, _data: &[u8]) {
        self.command_count += 1;
    }

    fn report_errror(&mut self, _error: ParseError, _level: icy_parser_core::ErrorLevel) {
        self.error_count += 1;
    }
}

/// Generate various fuzzy input patterns
fn generate_fuzz_patterns() -> Vec<Vec<u8>> {
    let mut patterns = Vec::new();

    // Random bytes
    patterns.push((0..256).map(|i| i as u8).collect());

    // Escape sequence fragments
    patterns.push(b"\x1B".to_vec());
    patterns.push(b"\x1B[".to_vec());
    patterns.push(b"\x1B[;".to_vec());
    patterns.push(b"\x1B[;;;;;;;".to_vec());
    patterns.push(b"\x1B[999999999999999999999".to_vec());

    // CSI sequences with invalid terminators
    for i in 0..=255u8 {
        patterns.push(vec![0x1B, b'[', b'1', i]);
    }

    // All control characters
    patterns.push((0..32).collect());

    // High bytes (UTF-8-like sequences)
    patterns.push((128..256).map(|i| i as u8).collect());

    // Mixed valid and invalid
    patterns.push(b"\x1B[1mHello\x1B[99999999".to_vec());
    patterns.push(b"Text\x1B\x1B\x1B[[\x1B]]]".to_vec());

    // RIP-like sequences
    patterns.push(b"!|".to_vec());
    patterns.push(b"!|1".to_vec());
    patterns.push(b"!|\x00\x00\x00".to_vec());
    patterns.push(b"!|#\x00\x00\x00".to_vec());

    // AVATAR-like sequences
    patterns.push(b"\x16".to_vec());
    patterns.push(b"\x16\x01".to_vec());
    patterns.push(b"\x19".to_vec());
    patterns.push(b"\x19AA".to_vec());

    // PCBoard-like sequences
    patterns.push(b"@X".to_vec());
    patterns.push(b"@X0".to_vec());
    patterns.push(b"@XFF".to_vec());

    // Renegade-like sequences
    patterns.push(b"|".to_vec());
    patterns.push(b"|0".to_vec());
    patterns.push(b"|99".to_vec());

    // CtrlA-like sequences
    patterns.push(b"\x01".to_vec());
    patterns.push(b"\x01\x00".to_vec());

    // IGS-like sequences
    patterns.push(b"G#".to_vec());
    patterns.push(b"G#B".to_vec());
    patterns.push(b"G#B1,2,3,4,5:".to_vec());

    // VT52-like sequences
    patterns.push(b"\x1BY".to_vec());
    patterns.push(b"\x1BY  ".to_vec());

    // SkyPix-like sequences
    patterns.push(b"\x1B(".to_vec());
    patterns.push(b"\x1B(0".to_vec());

    // Mode7-like sequences
    for i in 128..160u8 {
        patterns.push(vec![i]);
    }

    // Viewdata-like sequences
    patterns.push((128..160).map(|i| i as u8).collect());

    // PETSCII-like sequences
    patterns.push(b"\x93".to_vec()); // CLR
    patterns.push(b"\x13".to_vec()); // HOME
    patterns.push(b"\x1C".to_vec()); // Color

    // ATASCII-like sequences
    patterns.push(b"\x1B".to_vec());
    patterns.push(b"\x1B*".to_vec());

    // Very long sequences
    patterns.push(vec![b'A'; 10000]);
    patterns.push(vec![0x1B; 1000]);
    patterns.push(b"\x1B[".repeat(500));

    // NULL bytes
    patterns.push(vec![0; 100]);

    // Alternating patterns
    patterns.push((0..1000).map(|i| if i % 2 == 0 { 0x1B } else { b'[' }).collect());

    // Random interleaving of escape sequences and text
    patterns.push(b"Hello\x1B[1mWorld\x1B[2J\x1B[HTest\x1B[999999999".to_vec());

    patterns
}

macro_rules! fuzz_test_parser {
    ($test_name:ident, $parser_type:ty) => {
        #[test]
        fn $test_name() {
            let patterns = generate_fuzz_patterns();

            for (idx, pattern) in patterns.iter().enumerate() {
                let mut parser = <$parser_type>::new();
                let mut sink = FuzzSink::new();

                // Should not panic regardless of input
                parser.parse(pattern, &mut sink);

                // Test parsing in small chunks
                let mut parser2 = <$parser_type>::new();
                let mut sink2 = FuzzSink::new();
                for chunk in pattern.chunks(3) {
                    parser2.parse(chunk, &mut sink2);
                }

                // Test single byte at a time
                let mut parser3 = <$parser_type>::new();
                let mut sink3 = FuzzSink::new();
                for &byte in pattern.iter() {
                    parser3.parse(&[byte], &mut sink3);
                }

                // If we got here without panicking, the test passed
                // We don't care about correctness in fuzzing, just no crashes
                assert!(true, "Parser survived pattern {}", idx);
            }
        }
    };
}

// Generate fuzz tests for all parsers
fuzz_test_parser!(fuzz_ansi_parser, AnsiParser);
fuzz_test_parser!(fuzz_avatar_parser, AvatarParser);
fuzz_test_parser!(fuzz_pcboard_parser, PcBoardParser);
fuzz_test_parser!(fuzz_ctrla_parser, CtrlAParser);
fuzz_test_parser!(fuzz_renegade_parser, RenegadeParser);
fuzz_test_parser!(fuzz_atascii_parser, AtasciiParser);
fuzz_test_parser!(fuzz_petscii_parser, PetsciiParser);
fuzz_test_parser!(fuzz_viewdata_parser, ViewdataParser);
fuzz_test_parser!(fuzz_mode7_parser, Mode7Parser);
fuzz_test_parser!(fuzz_skypix_parser, SkypixParser);

// RIP parser needs special handling since it's more complex
#[test]
fn fuzz_rip_parser() {
    use icy_parser_core::RipParser;

    let patterns = generate_fuzz_patterns();

    for (idx, pattern) in patterns.iter().enumerate() {
        let mut parser = RipParser::new();
        let mut sink = FuzzSink::new();

        // Should not panic regardless of input
        parser.parse(pattern, &mut sink);

        // Test parsing in small chunks
        let mut parser2 = RipParser::new();
        let mut sink2 = FuzzSink::new();
        for chunk in pattern.chunks(3) {
            parser2.parse(chunk, &mut sink2);
        }

        assert!(true, "RipParser survived pattern {}", idx);
    }
}

// IGS parser test
#[test]
fn fuzz_igs_parser() {
    use icy_parser_core::IgsParser;

    let patterns = generate_fuzz_patterns();

    for (idx, pattern) in patterns.iter().enumerate() {
        let mut parser = IgsParser::new();
        let mut sink = FuzzSink::new();

        parser.parse(pattern, &mut sink);

        // Test byte by byte
        let mut parser2 = IgsParser::new();
        let mut sink2 = FuzzSink::new();
        for &byte in pattern.iter() {
            parser2.parse(&[byte], &mut sink2);
        }

        assert!(true, "IgsParser survived pattern {}", idx);
    }
}

// Test with completely random data
#[test]
fn fuzz_all_parsers_with_random() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Use deterministic "random" based on test name for reproducibility
    let mut hasher = DefaultHasher::new();
    "fuzz_all_parsers_with_random".hash(&mut hasher);
    let mut seed = hasher.finish();

    // Simple PRNG (not cryptographically secure, just for testing)
    let next_byte = |s: &mut u64| -> u8 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*s >> 32) as u8
    };

    for _ in 0..100 {
        let len = (next_byte(&mut seed) as usize) + 1;
        let random_bytes: Vec<u8> = (0..len).map(|_| next_byte(&mut seed)).collect();

        // Test each parser with this random data
        macro_rules! test_parser {
            ($parser_type:ty) => {{
                let mut parser = <$parser_type>::new();
                let mut sink = FuzzSink::new();
                parser.parse(&random_bytes, &mut sink);
            }};
        }

        test_parser!(AnsiParser);
        test_parser!(AvatarParser);
        test_parser!(PcBoardParser);
        test_parser!(CtrlAParser);
        test_parser!(RenegadeParser);
        test_parser!(AtasciiParser);
        test_parser!(PetsciiParser);
        test_parser!(ViewdataParser);
        test_parser!(Mode7Parser);
        test_parser!(SkypixParser);

        // Special handling for parsers with their own types
        {
            use icy_parser_core::RipParser;
            let mut parser = RipParser::new();
            let mut sink = FuzzSink::new();
            parser.parse(&random_bytes, &mut sink);
        }

        {
            use icy_parser_core::IgsParser;
            let mut parser = IgsParser::new();
            let mut sink = FuzzSink::new();
            parser.parse(&random_bytes, &mut sink);
        }
    }
}

// Test edge cases in numeric parsing
#[test]
fn fuzz_numeric_overflow_cases() {
    let overflow_patterns = vec![
        // Max u16 in various bases
        b"\x1B[65535m".to_vec(),
        b"\x1B[65536m".to_vec(),
        b"\x1B[99999999999999999999m".to_vec(),
        // Multiple huge numbers
        b"\x1B[999999999;888888888;777777777H".to_vec(),
        // Base36 overflow (RIP)
        b"!|#ZZZZZZZZZZZZZZZZZZ".to_vec(),
        // Long digit sequences
        vec![0x1B, b'[']
            .into_iter()
            .chain(std::iter::repeat(b'9').take(1000))
            .chain(std::iter::once(b'm'))
            .collect(),
    ];

    for pattern in &overflow_patterns {
        let mut parser = AnsiParser::new();
        let mut sink = FuzzSink::new();
        parser.parse(pattern, &mut sink);

        // RIP parser for base36 cases
        use icy_parser_core::RipParser;
        let mut rip_parser = RipParser::new();
        let mut rip_sink = FuzzSink::new();
        rip_parser.parse(pattern, &mut rip_sink);
    }
}

// Test state machine edge cases
#[test]
fn fuzz_state_transitions() {
    let state_patterns = vec![
        // Incomplete sequences
        b"\x1B".to_vec(),
        b"\x1B[".to_vec(),
        b"\x1B]".to_vec(),
        b"\x1B_".to_vec(),
        b"\x1BP".to_vec(),
        // Nested escapes
        b"\x1B[\x1B[\x1B[".to_vec(),
        b"\x1B]\x1B]\x1B]".to_vec(),
        // Interleaved sequences
        b"\x1B[1\x1B[2m".to_vec(),
        b"!|\x1B[1m#".to_vec(),
        // Sequences with nulls
        b"\x1B[\x00\x00m".to_vec(),
        b"!|\x00#\x00".to_vec(),
    ];

    for pattern in &state_patterns {
        macro_rules! test_parser {
            ($parser_type:ty) => {{
                let mut parser = <$parser_type>::new();
                let mut sink = FuzzSink::new();
                parser.parse(pattern, &mut sink);
            }};
        }

        test_parser!(AnsiParser);
        test_parser!(AvatarParser);

        use icy_parser_core::RipParser;
        let mut rip_parser = RipParser::new();
        let mut rip_sink = FuzzSink::new();
        rip_parser.parse(pattern, &mut rip_sink);
    }
}
