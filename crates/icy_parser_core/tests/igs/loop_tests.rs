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
        assert_eq!(
            generated, arg,
            "Round-trip failed: expected '{}', got '{}'",
            arg, generated
        );
    } else {
        panic!("Expected single loop command, got {}", sink.igs_commands.len());
    }
}

// ========================================
// Basic Loop Tests
// ========================================

#[test]
fn test_loop_simple() {
    test_roundtrip("G#&>0,198,6,0,L,4,0,0,x,y:");
}

#[test]
fn test_loop_basic_with_constants() {
    test_roundtrip("G#&>1,10,1,0,L,4,10,20,30,40:");
}

#[test]
fn test_loop_with_delay() {
    test_roundtrip("G#&>0,100,5,200,L,4,0,0,x,y:");
}

#[test]
fn test_loop_reverse_direction() {
    test_roundtrip("G#&>100,0,10,0,L,4,x,y,200,200:");
}

#[test]
fn test_loop_negative_step() {
    test_roundtrip("G#&>100,1,-10,0,L,2,x,y:");
}

// ========================================
// Step Variables (x, y)
// ========================================

#[test]
fn test_loop_only_step_forward() {
    test_roundtrip("G#&>0,100,10,0,L,4,x,x,x,x:");
}

#[test]
fn test_loop_only_step_reverse() {
    test_roundtrip("G#&>0,100,10,0,L,4,y,y,y,y:");
}

#[test]
fn test_loop_mixed_step_variables() {
    test_roundtrip("G#&>0,50,5,0,L,6,x,y,x,y,x,y:");
}

#[test]
fn test_loop_step_with_constants() {
    test_roundtrip("G#&>5,25,2,0,L,6,10,x,20,y,30,40:");
}

// ========================================
// Random Variables (r)
// ========================================

#[test]
fn test_loop_with_random() {
    test_roundtrip("G#&>1,10,1,0,S,4,r,r,50,50:");
}

#[test]
fn test_loop_all_random() {
    test_roundtrip("G#&>0,20,2,0,L,4,r,r,r,r:");
}

#[test]
fn test_loop_mixed_random_and_constants() {
    test_roundtrip("G#&>1,5,1,0,F,2,r,100:");
}

#[test]
fn test_loop_mixed_random_and_step() {
    test_roundtrip("G#&>0,100,10,0,L,4,x,r,y,r:");
}

// ========================================
// Expression Tests (+, -, !)
// ========================================

#[test]
fn test_loop_expr_add() {
    test_roundtrip("G#&>0,10,1,0,L,4,x,y,+10,+20:");
}

#[test]
fn test_loop_expr_subtract() {
    test_roundtrip("G#&>0,10,1,0,L,4,x,y,-10,-20:");
}

#[test]
fn test_loop_expr_subtract_step() {
    test_roundtrip("G#&>0,10,1,0,L,4,x,y,!100,!200:");
}

#[test]
fn test_loop_expr_all_operators() {
    test_roundtrip("G#&>0,10,1,0,L,6,+5,-10,!50,x,y,100:");
}

#[test]
fn test_loop_expr_negative_values() {
    test_roundtrip("G#&>0,10,1,0,L,4,+-10,--20,!-30,x:");
}

#[test]
fn test_loop_expr_large_values() {
    test_roundtrip("G#&>0,10,1,0,L,3,+999,!9999,-888:");
}

// ========================================
// Group Separator Tests (:)
// ========================================

#[test]
fn test_loop_with_separators() {
    test_roundtrip("G#&>0,10,1,0,L,4,10:20:30:40:");
}

#[test]
fn test_loop_separators_between_expressions() {
    test_roundtrip("G#&>0,10,1,0,L,4,x:y:+10:-20:");
}

#[test]
fn test_loop_separators_mixed() {
    test_roundtrip("G#&>1,5,1,0,L,6,10:x:20:y:+5:-3:");
}

#[test]
fn test_loop_multiple_consecutive_separators() {
    test_roundtrip("G#&>0,10,1,0,L,2,10::20:");
}

// ========================================
// XOR Stepping Modifier (|)
// ========================================

#[test]
fn test_loop_xor_modifier() {
    test_roundtrip("G#&>198,0,2,0,G|4,2,6,x,x:");
}

#[test]
fn test_loop_xor_with_line() {
    test_roundtrip("G#&>0,100,5,0,L|4,x,y,100,200:");
}

#[test]
fn test_loop_xor_with_expressions() {
    test_roundtrip("G#&>1,50,1,0,L|4,+10,x,-20,y:");
}


// ========================================
// Chain Gang Tests (>...@)
// ========================================

#[test]
fn test_loop_chain_gang_simple() {
    test_roundtrip("G#&>0,3,1,0,>CL@,4,x,y,100,200:");
}

#[test]
fn test_loop_chain_gang_three_commands() {
    test_roundtrip("G#&>0,10,2,0,>CLA@,6,x,y,100,200,10,20:");
}

#[test]
fn test_loop_chain_gang_long() {
    test_roundtrip("G#&>0,5,1,0,>CLABEFR@,2,x,y:");
}

#[test]
fn test_loop_chain_gang_with_expressions() {
    test_roundtrip("G#&>0,10,1,0,>CL@,6,x,y,+10,-20,!100,r:");
}

#[test]
fn test_loop_chain_gang_with_xor() {
    // Note: In chain gang, @ is part of the chain terminator, | is the modifier
    // The parser produces >CL@,0,16,... which suggests @ ends the chain before param_count
    test_roundtrip("G#&>0,636,4,0,>CL@,0,16,0,1,319,99,x,0,0,1,319,99,+2,0,0,0,0,0:");
}

// ========================================
// Different Command Types
// ========================================

#[test]
fn test_loop_grab_command() {
    test_roundtrip("G#&>0,100,10,0,G,8,2,3,x,x,2,3,y,y:");
}

#[test]
fn test_loop_color_command() {
    test_roundtrip("G#&>0,15,1,0,C,2,1,x:");
}

#[test]
fn test_loop_box_command() {
    test_roundtrip("G#&>0,100,5,0,B,5,x,y,+100,+100,1:");
}

#[test]
fn test_loop_circle_command() {
    test_roundtrip("G#&>0,50,5,0,O,3,x,y,50:");
}

#[test]
fn test_loop_ellipse_command() {
    test_roundtrip("G#&>0,100,10,0,Q,4,x,y,100,50:");
}

#[test]
fn test_loop_filled_rect_command() {
    test_roundtrip("G#&>0,200,20,0,Z,4,x,y,+50,+50:");
}

#[test]
fn test_loop_write_text_command() {
    test_roundtrip("G#&>20,140,20,0,W,2,x,50:");
}

#[test]
fn test_loop_plot_command() {
    test_roundtrip("G#&>0,100,10,0,P,2,x,y:");
}

#[test]
fn test_loop_drawto_command() {
    test_roundtrip("G#&>10,200,5,0,D,2,x,y:");
}

#[test]
fn test_loop_flood_fill_command() {
    test_roundtrip("G#&>0,100,20,0,F,2,x,y:");
}

// ========================================
// Edge Cases
// ========================================

#[test]
fn test_loop_zero_params() {
    test_roundtrip("G#&>1,10,1,0,s,0:");
}

#[test]
fn test_loop_single_param() {
    test_roundtrip("G#&>0,10,1,0,C,1,x:");
}

#[test]
fn test_loop_many_params() {
    test_roundtrip("G#&>0,5,1,0,L,10,x,y,+10,+20,r,50,100,-5,!200,y:");
}

#[test]
fn test_loop_zero_step_size() {
    // This technically creates infinite loop, but should parse/display correctly
    test_roundtrip("G#&>5,10,0,0,L,2,10,20:");
}

#[test]
fn test_loop_from_equals_to() {
    test_roundtrip("G#&>50,50,1,0,L,2,10,20:");
}

#[test]
fn test_loop_large_range() {
    test_roundtrip("G#&>0,9999,100,0,L,2,x,y:");
}

#[test]
fn test_loop_negative_range() {
    test_roundtrip("G#&>-100,100,10,0,L,2,x,y:");
}

#[test]
fn test_loop_all_negative() {
    test_roundtrip("G#&>-50,-10,5,0,L,2,x,y:");
}

// ========================================
// Complex Real-World Examples
// ========================================

#[test]
fn test_loop_complex_line_drawing() {
    test_roundtrip("G#&>85,300,5,0,D,24,340,10,340,60,420,60,420,85,340,85,340,180,85,180,x,85,220,85,220,60,x,60,x,10:");
}

#[test]
fn test_loop_color_cycling() {
    test_roundtrip("G#&>0,15,1,0,C,2,2,x:");
}

#[test]
fn test_loop_grab_with_stepping() {
    test_roundtrip("G#&>0,220,4,0,G,16,2,3,x,x,2,3,y,y,2,3,x,y,2,3,y,x:");
}

#[test]
fn test_loop_spray_paint_effect() {
    test_roundtrip("G#&>0,300,20,0,X,5,0,x,10,200,100:");
}

// ========================================
// Mixed Parameter Types in One Loop
// ========================================

#[test]
fn test_loop_all_token_types() {
    test_roundtrip("G#&>1,10,1,0,L,10,10,x,y,r,+5,-3,!100,20,30,40:");
}

#[test]
fn test_loop_all_token_types_with_separators() {
    test_roundtrip("G#&>1,10,1,0,L,10,10:x:y:r:+5:-3:!100:20:30:40:");
}

// ========================================
// Stress Tests
// ========================================

#[test]
fn test_loop_very_long_param_list() {
    test_roundtrip("G#&>0,2,1,0,L,20,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20:");
}

#[test]
fn test_loop_all_expressions() {
    test_roundtrip("G#&>0,10,1,0,L,8,+1,+2,+3,+4,-5,-6,!7,!8:");
}

#[test]
fn test_loop_alternating_x_y() {
    test_roundtrip("G#&>0,100,10,0,L,8,x,y,x,y,x,y,x,y:");
}

#[test]
fn test_loop_max_delay() {
    test_roundtrip("G#&>0,10,1,9999,L,2,x,y:");
}

#[test]
fn test_loop_with_zero_values() {
    test_roundtrip("G#&>0,0,0,0,L,4,0,0,0,0:");
}

// ========================================
// Line Continuation Tests (_)
// These test that _ underscore for line continuation is parsed correctly.
// The _ is a parse-time feature only and gets normalized away in the output.
// ========================================

#[test]
fn test_loop_line_continuation_simple() {
    // The _ replaces the first digit for line continuation during parsing
    // After parsing, it's normalized to the actual value without _
    let input = "G#&>0,10,1,0,L,4,100,200,_300,400:";
    let expected = "G#&>0,10,1,0,L,4,100,200,300,400:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected, "Line continuation should be normalized");
}

#[test]
fn test_loop_line_continuation_multiple() {
    // Multiple line continuations in sequence
    let input = "G#&>0,5,1,0,L,8,10,20,_30,_40,_50,60,_70,80:";
    let expected = "G#&>0,5,1,0,L,8,10,20,30,40,50,60,70,80:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_with_expressions() {
    // Line continuation with expressions
    let input = "G#&>0,10,1,0,L,6,x,y,_+10,_-20,_!100,50:";
    let expected = "G#&>0,10,1,0,L,6,x,y,+10,-20,!100,50:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_with_step_vars() {
    // Line continuation with step variables
    let input = "G#&>0,100,10,0,L,6,100,_x,200,_y,_x,_y:";
    let expected = "G#&>0,100,10,0,L,6,100,x,200,y,x,y:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_complex() {
    // Based on the spec example with line continuation
    let input = "G#&>85,300,5,0,D,24,340,10,340,60,420,60,420,85,_340,85,340,180,85,180,x,85,220,85,220,60,x,60,x,10:";
    let expected = "G#&>85,300,5,0,D,24,340,10,340,60,420,60,420,85,340,85,340,180,85,180,x,85,220,85,220,60,x,60,x,10:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_with_separators() {
    // Line continuation combined with group separators
    let input = "G#&>0,10,1,0,L,6,100:200:_300:_400:500:600:";
    let expected = "G#&>0,10,1,0,L,6,100:200:300:400:500:600:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_all_params() {
    // Every parameter uses line continuation
    let input = "G#&>0,5,1,0,L,4,_10,_20,_30,_40:";
    let expected = "G#&>0,5,1,0,L,4,10,20,30,40:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_with_random() {
    // Line continuation with random variables (r doesn't use _, but testing mixed)
    let input = "G#&>0,10,1,0,L,6,r,r,100,_200,r,_r:";
    let expected = "G#&>0,10,1,0,L,6,r,r,100,200,r,r:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_negative_values() {
    // Line continuation with negative values
    let input = "G#&>0,10,1,0,L,4,_-100,_-200,100,200:";
    let expected = "G#&>0,10,1,0,L,4,-100,-200,100,200:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_mixed_all() {
    // Complex mix: continuation, expressions, steps, random, separators
    let input = "G#&>0,10,1,0,L,12,100:_x:_y:_+50:_-30:_!100:_r:200:x:y:+10:-5:";
    let expected = "G#&>0,10,1,0,L,12,100:x:y:+50:-30:!100:r:200:x:y:+10:-5:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_at_start() {
    // Line continuation on the very first parameter
    let input = "G#&>0,10,1,0,L,4,_100,200,300,400:";
    let expected = "G#&>0,10,1,0,L,4,100,200,300,400:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_drawto_example() {
    // Real-world example with Drawto command
    let input = "G#&>0,100,10,0,D,8,x,y,_+10,_+10,_x,_y,100,100:";
    let expected = "G#&>0,100,10,0,D,8,x,y,+10,+10,x,y,100,100:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_write_text() {
    // Line continuation with write text command
    let input = "G#&>0,50,10,0,W,4,x,_y,_100,_200:";
    let expected = "G#&>0,50,10,0,W,4,x,y,100,200:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_chain_gang() {
    // Line continuation with chain gang
    let input = "G#&>0,10,2,0,>CL@,8,x,_y,_100,_200,_x,_y,300,400:";
    let expected = "G#&>0,10,2,0,>CL@,8,x,y,100,200,x,y,300,400:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

#[test]
fn test_loop_line_continuation_very_long() {
    // Simulating DEGAS conversion with many continued parameters
    let input = "G#&>0,2,1,0,L,20,_1,_2,_3,_4,_5,_6,_7,_8,_9,_10,_11,_12,_13,_14,_15,_16,_17,_18,_19,_20:";
    let expected = "G#&>0,2,1,0,L,20,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20:";
    
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, expected);
}

// ========================================
// W@ Text Loop Tests
// ========================================

#[test]
fn test_loop_write_text_single_string() {
    // W@ with single text string parameter
    test_roundtrip("G#&>20,40,20,0,W@1,Item 1@:");
}

#[test]
fn test_loop_write_text_multiple_strings() {
    // W@ with multiple text strings from spec example
    test_roundtrip("G#&>20,140,20,0,W@3,A. Item 1@B. Item 2@C. Item 3@:");
}

#[test]
fn test_loop_write_text_empty_string() {
    // W@ with empty text string
    test_roundtrip("G#&>0,20,10,0,W@1,@:");
}

#[test]
fn test_loop_write_text_with_spaces() {
    // W@ with text containing spaces
    test_roundtrip("G#&>10,30,10,0,W@1,Hello World@:");
}

#[test]
fn test_loop_write_text_with_punctuation() {
    // W@ with text containing special characters
    test_roundtrip("G#&>0,60,20,0,W@2,Item #1: Test!@Item #2: Check?@:");
}

#[test]
fn test_loop_write_text_xor_stepping() {
    // W@ with XOR stepping modifier (correct order is |@ not @|)
    test_roundtrip("G#&>0,40,20,0,W|@1,Text@:");
}

#[test]
fn test_loop_write_text_multiline() {
    // W@ with newline in text (note: newlines are preserved)
    let input = "G#&>0,40,20,0,W@,1,Line1\nLine2@:";
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();
    parser.parse(input.as_bytes(), &mut sink);
    
    assert_eq!(sink.igs_commands.len(), 1);
    let generated = format!("{}", sink.igs_commands[0]);
    assert_eq!(generated, input);
}

#[test]
fn test_loop_write_text_many_texts() {
    // W@ with more text strings
    test_roundtrip("G#&>0,100,10,0,W@,5,A@B@C@D@E@:");
}

#[test]
fn test_loop_write_text_long_text() {
    // W@ with longer text content
    test_roundtrip("G#&>10,50,20,0,W@,1,This is a longer text string for testing@:");
}

#[test]
fn test_loop_write_text_numbers_in_text() {
    // W@ with numeric content in text
    test_roundtrip("G#&>0,30,10,0,W@,1,Value: 123@:");
}

#[test]
fn test_loop_from_spec() {
    // W@ with numeric content in text
    test_roundtrip("G#&>20,140,20,0,W@2,0,x,A. Item 1@
B. Item 2@
C. Item 3@
D. Item 4@
E. Item 5@
F. Item 6@
G. Item 7@:");
}

