use icy_parser_core::{CommandParser, CommandSink, IgsCommand, IgsParser, TerminalCommand, TerminalRequest};

struct TestSink {
    igs_commands: Vec<IgsCommand>,
}

impl TestSink {
    fn new() -> Self {
        Self { igs_commands: Vec::new() }
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, _text: &[u8]) {}
    fn emit(&mut self, _cmd: TerminalCommand) {}
    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.igs_commands.push(cmd);
    }
    fn request(&mut self, _request: TerminalRequest) {}
}

// Drawing commands

#[test]
fn test_box() {
    test_roundtrip("G#B>0,0,100,100,0:");
}

#[test]
fn test_box_rounded() {
    test_roundtrip("G#B>10,20,150,80,1:");
}

#[test]
fn test_line() {
    test_roundtrip("G#L>0,0,300,150:");
}

#[test]
fn test_line_drawto() {
    test_roundtrip("G#D>149,99:");
}

#[test]
fn test_circle() {
    test_roundtrip("G#O>300,100,75:");
}

#[test]
fn test_ellipse() {
    test_roundtrip("G#Q>300,100,200,60:");
}

#[test]
fn test_arc() {
    test_roundtrip("G#K>300,99,75,90,180:");
}

#[test]
fn test_polyline() {
    test_roundtrip("G#z>3,100,0,150,100,50,100:");
}

#[test]
fn test_polyfill() {
    test_roundtrip("G#f>3,100,0,150,100,50,100:");
}

#[test]
fn test_flood_fill() {
    test_roundtrip("G#F>600,0:");
}

#[test]
fn test_polymarker_plot() {
    test_roundtrip("G#P>149,99:");
}

// Color/Style commands

#[test]
fn test_color_set() {
    test_roundtrip("G#C>0,2:");
}

#[test]
fn test_color_set_line() {
    test_roundtrip("G#C>1,5:");
}

#[test]
fn test_attributes_for_fills() {
    test_roundtrip("G#A>1,1,0:");
}

#[test]
fn test_attributes_pattern() {
    test_roundtrip("G#A>2,12,1:");
}

#[test]
fn test_line_style() {
    test_roundtrip("G#T>2,1,1:");
}

#[test]
fn test_line_style_arrows() {
    test_roundtrip("G#T>2,1,50:");
}

#[test]
fn test_set_pen_color() {
    test_roundtrip("G#S>1,0,0,7:");
}

#[test]
fn test_drawing_mode() {
    test_roundtrip("G#M>1:");
}

#[test]
fn test_drawing_mode_xor() {
    test_roundtrip("G#M>3:");
}

#[test]
fn test_hollow_set_on() {
    test_roundtrip("G#H>1:");
}

#[test]
fn test_hollow_set_off() {
    test_roundtrip("G#H>0:");
}

// Text commands

#[test]
fn test_write_text() {
    test_roundtrip("G#W>50,100,DEVO E-Z Listening Disc@");
}

#[test]
fn test_write_text_simple() {
    test_roundtrip("G#W>20,50,Chain@");
}

#[test]
fn test_text_effects() {
    test_roundtrip("G#E>8,18,1:");
}

#[test]
fn test_text_effects_bold() {
    test_roundtrip("G#E>1,10,0:");
}

#[test]
fn test_text_effects_rotated() {
    test_roundtrip("G#E>0,18,2:");
}

// Special commands

#[test]
fn test_bells_and_whistles() {
    test_roundtrip("G#b>0:");
}

#[test]
fn test_bells_explosion() {
    test_roundtrip("G#b>6:");
}

#[test]
fn test_bells_alter_effect() {
    test_roundtrip("G#b>20,0,9,0,0,0,800:");
}

#[test]
fn test_graphic_scaling_on() {
    test_roundtrip("G#g>1:");
}

#[test]
fn test_graphic_scaling_off() {
    test_roundtrip("G#g>0:");
}

#[test]
fn test_grab_screen_to_screen() {
    test_roundtrip("G#G>0,3,0,0,100,100,100,50:");
}

#[test]
fn test_grab_screen_to_memory() {
    test_roundtrip("G#G>1,3,0,0,100,100:");
}

#[test]
fn test_grab_memory_to_screen() {
    test_roundtrip("G#G>2,3,200,50:");
}

#[test]
fn test_initialize() {
    test_roundtrip("G#I>0:");
}

#[test]
fn test_initialize_palette() {
    test_roundtrip("G#I>3:");
}

#[test]
fn test_initialize_resolution() {
    test_roundtrip("G#I>5:");
}

#[test]
fn test_elliptical_arc() {
    test_roundtrip("G#J>0,199,400,600,0,270:");
}

#[test]
fn test_cursor_off() {
    test_roundtrip("G#k>0:");
}

#[test]
fn test_cursor_on() {
    test_roundtrip("G#k>1:");
}

#[test]
fn test_chip_music() {
    test_roundtrip("G#n>13,1,16,60,200,2:");
}

#[test]
fn test_noise() {
    test_roundtrip("G#N>2:");
}

#[test]
fn test_rounded_rectangles() {
    test_roundtrip("G#U>100,0,300,150,1:");
}

#[test]
fn test_rounded_rectangles_filled() {
    test_roundtrip("G#U>100,0,300,150,0:");
}

#[test]
fn test_pie_slice() {
    test_roundtrip("G#V>50,50,100,180,270:");
}

#[test]
fn test_elliptical_pie_slice() {
    test_roundtrip("G#Y>80,80,100,200,0,180:");
}

#[test]
fn test_filled_rectangle() {
    test_roundtrip("G#Z>10,10,200,100:");
}

#[test]
fn test_input_command() {
    test_roundtrip("G#<>1,0,1:");
}

#[test]
fn test_ask_ig_version() {
    test_roundtrip("G#?>0:");
}

#[test]
fn test_ask_ig_resolution() {
    test_roundtrip("G#?>3:");
}

#[test]
fn test_screen_clear() {
    test_roundtrip("G#s>0:");
}

#[test]
fn test_screen_clear_to_bottom() {
    test_roundtrip("G#s>2:");
}

#[test]
fn test_set_resolution_low() {
    test_roundtrip("G#R>0,0:");
}

#[test]
fn test_set_resolution_med_vdi() {
    test_roundtrip("G#R>1,3:");
}

#[test]
fn test_quick_pause() {
    test_roundtrip("G#t>2:");
}

#[test]
fn test_quick_pause_vsync() {
    test_roundtrip("G#q>180:");
}

#[test]
fn test_loop_command() {
    test_roundtrip("G#&>0,198,6,0,L,4,0,0,x,y:");
}

// Extended X commands

#[test]
fn test_spray_paint() {
    test_roundtrip("G#X>0,400,50,200,145,200:");
}

#[test]
fn test_set_color_register() {
    test_roundtrip("G#X>1,4,0:");
}

#[test]
fn test_set_random_range() {
    test_roundtrip("G#X>2,0,50:");
}

#[test]
fn test_set_random_range_r() {
    test_roundtrip("G#X>2,9,9,100:");
}

#[test]
fn test_right_mouse_macro() {
    test_roundtrip("G#X>3,0:");
}

#[test]
fn test_define_zone() {
    test_roundtrip("G#X>4,0,0,0,79,49,3,f/L:");
}

#[test]
fn test_define_zone_bug() {
    test_roundtrip("G#X>4,1,26,48,215,53,1,a:");
}

#[test]
fn test_define_zone_clear() {
    test_roundtrip("G#X>4,9999:");
}

#[test]
fn test_flow_control_off() {
    test_roundtrip("G#X>5,0:");
}

#[test]
fn test_flow_control_on() {
    test_roundtrip("G#X>5,1:");
}

#[test]
fn test_left_mouse_button() {
    test_roundtrip("G#X>6,0:");
}

#[test]
fn test_load_fill_pattern() {
    // Pattern with proper format: 16 lines × 17 chars (272 bytes total)
    // Each line: 16 pattern chars + '@' delimiter
    let pattern = concat!(
        "G#X>7,1,",
        "----------------@\n", // Line 1: all zeros -> 0x0000
        "----------------@\n", // Line 2: all zeros -> 0x0000
        "--------XX------@\n", // Line 3: two bits set -> 0x00C0
        "-------XXXX-----@\n", // Line 4: four bits set -> 0x01E0
        "------XXXXXX----@\n", // Line 5: six bits set -> 0x03F0
        "-----XXXXXXXX---@\n", // Line 6: eight bits set -> 0x07F8
        "----XXXXXXXXXX--@\n", // Line 7: ten bits set -> 0x0FFC
        "---XXXXXXXXXXXX-@\n", // Line 8: twelve bits set -> 0x1FFE
        "---XXXXXXXXXXXX-@\n", // Line 9: twelve bits set -> 0x1FFE
        "----XXXXXXXXXX--@\n", // Line 10: ten bits set -> 0x0FFC
        "-----XXXXXXXX---@\n", // Line 11: eight bits set -> 0x07F8
        "------XXXXXX----@\n", // Line 12: six bits set -> 0x03F0
        "-------XXXX-----@\n", // Line 13: four bits set -> 0x01E0
        "--------XX------@\n", // Line 14: two bits set -> 0x00C0
        "----------------@\n", // Line 15: all zeros -> 0x0000
        "----------------@",   // Line 16: all zeros -> 0x0000
    );

    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(pattern.as_bytes(), &mut sink);

    assert_eq!(sink.igs_commands.len(), 1, "Should parse one command");

    if let IgsCommand::LoadFillPattern { pattern: pat, data } = &sink.igs_commands[0] {
        assert_eq!(*pat, 1, "Pattern slot should be 1");
        assert_eq!(data.len(), 16, "Should have 16 pattern lines");

        // Verify the pattern values
        let expected = vec![
            0x0000, // ----------------
            0x0000, // ----------------
            0x00C0, // --------XX------
            0x01E0, // -------XXXX-----
            0x03F0, // ------XXXXXX----
            0x07F8, // -----XXXXXXXX---
            0x0FFC, // ----XXXXXXXXXX--
            0x1FFE, // ---XXXXXXXXXXXX-
            0x1FFE, // ---XXXXXXXXXXXX-
            0x0FFC, // ----XXXXXXXXXX--
            0x07F8, // -----XXXXXXXX---
            0x03F0, // ------XXXXXX----
            0x01E0, // -------XXXX-----
            0x00C0, // --------XX------
            0x0000, // ----------------
            0x0000, // ----------------
        ];

        for (i, (&actual, &expected)) in data.iter().zip(expected.iter()).enumerate() {
            assert_eq!(actual, expected, "Line {} mismatch: got 0x{:04X}, expected 0x{:04X}", i, actual, expected);
        }
    } else {
        panic!("Expected LoadFillPattern command");
    }

    // Test roundtrip - output should be compact form without newlines
    let expected_output = "G#X>7,1,----------------@----------------@--------XX------@-------XXXX-----@------XXXXXX----@-----XXXXXXXX---@----XXXXXXXXXX--@---XXXXXXXXXXXX-@---XXXXXXXXXXXX-@----XXXXXXXXXX--@-----XXXXXXXX---@------XXXXXX----@-------XXXX-----@--------XX------@----------------@----------------@:";
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(
        generated, expected_output,
        "Round-trip failed: expected '{}', got '{}'",
        expected_output, generated
    );
}

#[test]
fn test_rotate_color_registers() {
    test_roundtrip("G#X>8,3,15,20,10:");
}

#[test]
fn test_load_midi_buffer() {
    test_roundtrip("G#X>9,0:");
}

#[test]
fn test_set_drawto_begin() {
    test_roundtrip("G#X>10,100,50:");
}

#[test]
fn test_load_bitblit_memory() {
    test_roundtrip("G#X>11,0,0,49:");
}

#[test]
fn test_load_color_palette() {
    test_roundtrip("G#X>12,0,0,1911,1792,112:");
}

// VT52 commands - These now emit TerminalCommand, not IgsCommand
// The following tests are disabled because VT52 sequences should be handled
// as standard terminal commands, not IGS-specific commands

/*
#[test]
fn test_cursor_up() {
    test_roundtrip("\x1bA");
}

#[test]
fn test_cursor_down() {
    test_roundtrip("\x1bB");
}

#[test]
fn test_cursor_right() {
    test_roundtrip("\x1bC");
}

#[test]
fn test_cursor_left() {
    test_roundtrip("\x1bD");
}

#[test]
fn test_cursor_home() {
    test_roundtrip("\x1bH");
}

#[test]
fn test_clear_screen() {
    test_roundtrip("G#s>0:");
}

#[test]
fn test_clear_to_eol() {
    test_roundtrip("\x1bK");
}
*/

#[test]
fn test_clear_to_eos() {
    test_roundtrip("G#s>2:");
}

/*
#[test]
fn test_set_cursor_pos() {
    test_roundtrip("\x1bY*5"); // Row 10, Col 5
}

#[test]
fn test_set_foreground() {
    test_roundtrip("\x1bb\x0F");
}

#[test]
fn test_set_background() {
    test_roundtrip("\x1bc\x00");
}

#[test]
fn test_show_cursor() {
    test_roundtrip("G#k>1:");
}

#[test]
fn test_hide_cursor() {
    test_roundtrip("G#k>0:");
}

#[test]
fn test_save_cursor_pos() {
    test_roundtrip("\x1bj");
}

#[test]
fn test_restore_cursor_pos() {
    test_roundtrip("\x1bk");
}

#[test]
fn test_delete_line() {
    test_roundtrip("\x1bd\x04");
}

#[test]
fn test_insert_line() {
    test_roundtrip("\x1bi\x01");
}
*/

/*
#[test]
fn test_clear_line() {
    test_roundtrip("\x1bl");
}
*/

#[test]
fn test_position_cursor() {
    test_roundtrip("G#p>70,19:");
}

#[test]
fn test_inverse_video_on() {
    test_roundtrip("G#v>1:");
}

#[test]
fn test_inverse_video_off() {
    test_roundtrip("G#v>0:");
}

#[test]
fn test_line_wrap_on() {
    test_roundtrip("G#w>1:");
}

#[test]
fn test_line_wrap_off() {
    test_roundtrip("G#w>0:");
}

// Complex chained examples from spec

#[test]
fn test_chain_example() {
    test_roundtrip("G#I>0:k>0:s>4:");
}

#[test]
fn test_drawing_chain() {
    test_roundtrip("G#C>1,1:T>2,1,1:L>10,48,152,48:");
}

#[test]
fn test_text_chain() {
    test_roundtrip("G#E>0,18,0:C>3,2:W>20,50,Hello@");
}

// Helper function

fn test_roundtrip(arg: &str) {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Parse the input
    parser.parse(arg.as_bytes(), &mut sink);

    // Check that we got commands
    if sink.igs_commands.is_empty() {
        panic!("No IGS commands parsed from input: {}", arg);
    }

    // For single commands, verify exact roundtrip
    if sink.igs_commands.len() == 1 {
        let generated = format!("{}", sink.igs_commands[0]);
        assert_eq!(generated, arg, "Round-trip failed: expected '{}', got '{}'", arg, generated);
    } else {
        // For chained commands (using ':'), concatenate with proper chain formatting
        // First command gets full G# prefix, subsequent commands omit G#
        let mut combined = String::new();
        for (i, cmd) in sink.igs_commands.iter().enumerate() {
            let cmd_str = format!("{}", cmd);
            if i == 0 {
                // First command: use as-is (includes G#)
                combined.push_str(&cmd_str);
            } else {
                // Subsequent commands: strip G# prefix if present
                if let Some(stripped) = cmd_str.strip_prefix("G#") {
                    combined.push_str(stripped);
                } else {
                    combined.push_str(&cmd_str);
                }
            }
        }

        assert_eq!(combined, arg, "Round-trip failed for chain: expected '{}', got '{}'", arg, combined);
    }
}

// Additional tests for edge cases

#[test]
fn test_large_coordinates() {
    test_roundtrip("G#L>0,0,639,399:");
}

#[test]
fn test_polyline_many_points() {
    test_roundtrip("G#z>5,0,0,50,50,100,0,150,50,200,0:");
}

#[test]
fn test_text_with_special_chars() {
    test_roundtrip("G#W>10,10,Hello World!@");
}

#[test]
fn test_multiple_colors() {
    test_roundtrip("G#S>0,7,7,7:");
    test_roundtrip("G#S>15,0,0,0:");
}

#[test]
fn test_grab_screen_memory_to_memory() {
    test_roundtrip("G#G>4,3,50,50,75,75,150,100:");
}

#[test]
fn test_loop_with_parameters() {
    test_roundtrip("G#&>10,30,2,0,L,4,100,10,-10,600:");
}

#[test]
fn test_loop_bug1() {
    test_roundtrip("G#&>0,1,1,0,S,8,11,5,5,5:15,5,5,5:");
}

#[test]
fn test_loop_bug2() {
    test_roundtrip("G#&>0,40,1,0,G,8,0,3,!90,0,!90,27,+51,102:");
}

#[test]
fn test_inner_loop() {
    test_roundtrip("G#&>1,10,1,0,>Gq@,18,3,3,0,102,20,107,218,156,10,3,3,0,109,20,114,218,156,10:");
}

#[test]
#[ignore] // TODO: ChainGang with nested commands not yet fully supported by parser
fn test_inner_loop2() {
    test_roundtrip("G#&>1,10,1,0,>Gq@,22,0G3,3,0,102,20,107,218,156:1q10:0G3,3,0,109,20,114,218,156:1q10:");
}

// Query commands

#[test]
fn test_ask_version() {
    test_roundtrip("G#?>0:");
}

#[test]
fn test_ask_cursor_position_immediate() {
    test_roundtrip("G#?>1,0:");
}

#[test]
fn test_ask_cursor_position_polymarker() {
    test_roundtrip("G#?>1,1:");
}

#[test]
fn test_ask_cursor_position_arrow() {
    test_roundtrip("G#?>1,3:");
}

#[test]
fn test_ask_mouse_position_immediate() {
    test_roundtrip("G#?>2,0:");
}

#[test]
fn test_ask_mouse_position_hourglass() {
    test_roundtrip("G#?>2,4:");
}

#[test]
fn test_ask_mouse_position_crosshair() {
    test_roundtrip("G#?>2,10:");
}

#[test]
fn test_ask_resolution() {
    test_roundtrip("G#?>3:");
}

// LoadFillPattern tests

#[test]
fn test_load_fill_pattern_error_wrong_length() {
    // Test with incorrect buffer length (should be 272 bytes: 16 × 17)
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // This pattern is too short
    parser.parse(b"G#X>7,1,XXXXXXXXXXXXXXXX:", &mut sink);

    // Should not produce a command due to invalid length
    assert_eq!(sink.igs_commands.len(), 0, "Should reject pattern with wrong length");
}

#[test]
fn test_load_fill_pattern_error_missing_delimiter() {
    // Test with missing '@' delimiter
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Pattern with wrong delimiter (using '#' instead of '@')
    let pattern = concat!(
        "G#X>7,2,",
        "XXXXXXXXXXXXXXXX#", // Wrong delimiter
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        ":"
    );
    parser.parse(pattern.as_bytes(), &mut sink);

    // Should not produce a command due to invalid delimiter
    assert_eq!(sink.igs_commands.len(), 0, "Should reject pattern with wrong delimiter");
}

#[test]
fn test_load_fill_pattern_error_invalid_slot() {
    // Test with invalid pattern slot (>7)
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Valid pattern data but invalid slot number
    let pattern = concat!(
        "G#X>7,8,", // Slot 8 is invalid (only 0-7 allowed)
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        "----------------@",
        ":"
    );
    parser.parse(pattern.as_bytes(), &mut sink);

    // Should not produce a command due to invalid slot
    assert_eq!(sink.igs_commands.len(), 0, "Should reject pattern with invalid slot number");
}

#[test]
fn test_load_fill_pattern_all_set() {
    // Pattern with all bits set
    let pattern = concat!(
        "G#X>7,3,",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        "XXXXXXXXXXXXXXXX@",
        ":"
    );

    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(pattern.as_bytes(), &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);

    if let IgsCommand::LoadFillPattern { pattern: pat, data } = &sink.igs_commands[0] {
        assert_eq!(*pat, 3);
        assert_eq!(data.len(), 16);

        // All lines should be 0xFFFF
        for (i, &value) in data.iter().enumerate() {
            assert_eq!(value, 0xFFFF, "Line {} should be all 1s (0xFFFF), got 0x{:04X}", i, value);
        }
    } else {
        panic!("Expected LoadFillPattern command");
    }

    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, pattern);
}

#[test]
fn test_load_fill_pattern_lowercase_x() {
    // Test that lowercase 'x' also works
    let pattern = concat!(
        "G#X>7,4,",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        "xxxxxxxxxxxxxxxx@",
        ":"
    );
    // Note: Display will normalize to uppercase 'X'
    let expected = pattern.replace('x', "X");

    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(pattern.as_bytes(), &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected, "Round-trip should normalize lowercase to uppercase");
}

#[test]
fn test_load_fill_pattern_checkerboard() {
    // Checkerboard pattern
    let pattern = concat!(
        "G#X>7,5,",
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        "X-X-X-X-X-X-X-X-@", // 0xAAAA
        "-X-X-X-X-X-X-X-X@", // 0x5555
        ":"
    );

    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(pattern.as_bytes(), &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);

    if let IgsCommand::LoadFillPattern { pattern: pat, data } = &sink.igs_commands[0] {
        assert_eq!(*pat, 5);
        assert_eq!(data.len(), 16);

        // Checkerboard should alternate between 0xAAAA and 0x5555
        for (i, &value) in data.iter().enumerate() {
            let expected = if i % 2 == 0 { 0xAAAA } else { 0x5555 };
            assert_eq!(value, expected, "Line {} should be 0x{:04X}, got 0x{:04X}", i, expected, value);
        }
    } else {
        panic!("Expected LoadFillPattern command");
    }

    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, pattern);
}
