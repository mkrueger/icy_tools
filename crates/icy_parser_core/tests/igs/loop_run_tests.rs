use icy_parser_core::{CommandParser, CommandSink, IgsCommand, IgsParameter, IgsParser, ParameterBounds, TerminalCommand, TerminalRequest};

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

/// Helper to parse a loop command and execute it, returning the generated commands.
fn run_loop(input: &str) -> Vec<IgsCommand> {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Parse the loop command
    parser.parse(input.as_bytes(), &mut sink);

    // Should get exactly one Loop command
    assert_eq!(sink.igs_commands.len(), 1, "Expected exactly one loop command");

    // Extract the loop and run it
    let loop_cmd = sink.igs_commands.into_iter().next().unwrap();
    match loop_cmd {
        IgsCommand::Loop(data) => {
            let mut run_sink = TestSink::new();
            let bounds = ParameterBounds::default();
            data.run(&mut run_sink, &bounds);
            run_sink.igs_commands
        }
        _ => panic!("Expected Loop command, got {:?}", loop_cmd),
    }
}

// ========================================
// Basic Loop Execution Tests
// ========================================

#[test]
fn test_run_simple_line_loop() {
    // Loop that draws lines with x,y stepping
    let commands = run_loop("G#&>0,2,1,0,L,4,0,0,x,y:");

    // Should generate 3 line commands (iterations: 0, 1, 2)
    assert_eq!(commands.len(), 3);

    // Check first iteration: x=0, y=2
    match &commands[0] {
        IgsCommand::Line { x1, y1, x2, y2 } => {
            assert_eq!(*x1, 0.into());
            assert_eq!(*y1, 0.into());
            assert_eq!(*x2, 0.into());
            assert_eq!(*y2, 2.into());
        }
        _ => panic!("Expected Line command"),
    }

    // Check second iteration: x=1, y=1
    match &commands[1] {
        IgsCommand::Line { x1, y1, x2, y2 } => {
            assert_eq!(*x1, 0.into());
            assert_eq!(*y1, 0.into());
            assert_eq!(*x2, 1.into());
            assert_eq!(*y2, 1.into());
        }
        _ => panic!("Expected Line command"),
    }

    // Check third iteration: x=2, y=0
    match &commands[2] {
        IgsCommand::Line { x1, y1, x2, y2 } => {
            assert_eq!(*x1, 0.into());
            assert_eq!(*y1, 0.into());
            assert_eq!(*x2, 2.into());
            assert_eq!(*y2, 0.into());
        }
        _ => panic!("Expected Line command"),
    }
}

#[test]
fn test_run_loop_with_constants() {
    // Loop with constant parameters
    let commands = run_loop("G#&>0,2,1,0,L,4,10,20,30,40:");

    // Should generate 3 identical line commands
    assert_eq!(commands.len(), 3);

    for cmd in &commands {
        match cmd {
            IgsCommand::Line { x1, y1, x2, y2 } => {
                assert_eq!(*x1, 10.into());
                assert_eq!(*y1, 20.into());
                assert_eq!(*x2, 30.into());
                assert_eq!(*y2, 40.into());
            }
            _ => panic!("Expected Line command"),
        }
    }
}

#[test]
fn test_run_loop_step_forward_only() {
    // Loop using only x (step forward)
    let commands = run_loop("G#&>0,3,1,0,O,3,50,50,x:");

    // Should generate 4 circle commands with increasing radius
    assert_eq!(commands.len(), 4);

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Circle { x, y, radius } => {
                assert_eq!(*x, 50.into());
                assert_eq!(*y, 50.into());
                assert_eq!(*radius, (i as i32).into());
            }
            _ => panic!("Expected Circle command"),
        }
    }
}

#[test]
fn test_run_loop_step_reverse() {
    // Loop using only y (step reverse): y = to + from - x
    let commands = run_loop("G#&>0,3,1,0,O,3,50,50,y:");

    // Should generate 4 circle commands with decreasing radius
    assert_eq!(commands.len(), 4);

    let expected = [3, 2, 1, 0]; // y values for x=0,1,2,3 when from=0, to=3
    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Circle { x, y, radius } => {
                assert_eq!(*x, 50.into());
                assert_eq!(*y, 50.into());
                assert_eq!(*radius, expected[i].into());
            }
            _ => panic!("Expected Circle command"),
        }
    }
}

#[test]
fn test_run_loop_with_expressions_add() {
    // Loop with +N expression
    let commands = run_loop("G#&>0,2,1,0,L,4,x,x,+10,+20:");

    assert_eq!(commands.len(), 3);

    let expected = [(0, 0, 10, 20), (1, 1, 11, 21), (2, 2, 12, 22)];

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Line { x1, y1, x2, y2 } => {
                assert_eq!(*x1, expected[i].0.into());
                assert_eq!(*y1, expected[i].1.into());
                assert_eq!(*x2, expected[i].2.into());
                assert_eq!(*y2, expected[i].3.into());
            }
            _ => panic!("Expected Line command"),
        }
    }
}

#[test]
fn test_run_loop_with_expressions_subtract() {
    // Loop with -N expression
    let commands = run_loop("G#&>10,12,1,0,L,4,x,x,-5,-3:");

    assert_eq!(commands.len(), 3);

    let expected = [(10, 10, 5, 7), (11, 11, 6, 8), (12, 12, 7, 9)];

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Line { x1, y1, x2, y2 } => {
                assert_eq!(*x1, expected[i].0.into());
                assert_eq!(*y1, expected[i].1.into());
                assert_eq!(*x2, expected[i].2.into());
                assert_eq!(*y2, expected[i].3.into());
            }
            _ => panic!("Expected Line command"),
        }
    }
}

#[test]
fn test_run_loop_with_expressions_subtract_step() {
    // Loop with !N expression (N - step)
    let commands = run_loop("G#&>0,2,1,0,L,4,x,x,!100,!50:");

    assert_eq!(commands.len(), 3);

    let expected = [(0, 0, 100, 50), (1, 1, 99, 49), (2, 2, 98, 48)];

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Line { x1, y1, x2, y2 } => {
                assert_eq!(*x1, expected[i].0.into());
                assert_eq!(*y1, expected[i].1.into());
                assert_eq!(*x2, expected[i].2.into());
                assert_eq!(*y2, expected[i].3.into());
            }
            _ => panic!("Expected Line command"),
        }
    }
}

#[test]
fn test_run_loop_reverse_direction() {
    // Loop going backwards (from > to)
    let commands = run_loop("G#&>10,5,2,0,O,3,50,50,x:");

    // from=10, to=5, step=2 (negative since from > to)
    // iterations: 10, 8, 6
    assert_eq!(commands.len(), 3);

    let expected = [10, 8, 6];

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Circle { x, y, radius } => {
                assert_eq!(*x, 50.into());
                assert_eq!(*y, 50.into());
                assert_eq!(*radius, expected[i].into());
            }
            _ => panic!("Expected Circle command"),
        }
    }
}

#[test]
fn test_run_loop_negative_step() {
    // Loop with explicit negative step
    let commands = run_loop("G#&>10,5,-2,0,O,3,50,50,x:");

    // from=10, to=5, step=-2
    // iterations: 10, 8, 6
    assert_eq!(commands.len(), 3);

    let expected = [10, 8, 6];

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Circle { x, y, radius } => {
                assert_eq!(*x, 50.into());
                assert_eq!(*y, 50.into());
                assert_eq!(*radius, expected[i].into());
            }
            _ => panic!("Expected Circle command"),
        }
    }
}

#[test]
fn test_run_loop_box_command() {
    // Loop drawing boxes
    let commands = run_loop("G#&>0,2,1,0,B,5,x,x,+50,+50,0:");

    assert_eq!(commands.len(), 3);

    for (i, cmd) in commands.iter().enumerate() {
        let x = i as i32;
        match cmd {
            IgsCommand::Box { x1, y1, x2, y2, rounded } => {
                assert_eq!(*x1, x.into());
                assert_eq!(*y1, x.into());
                assert_eq!(*x2, (x + 50).into());
                assert_eq!(*y2, (x + 50).into());
                assert_eq!(*rounded, false);
            }
            _ => panic!("Expected Box command"),
        }
    }
}

#[test]
fn test_run_loop_color_set() {
    // Loop setting colors
    let commands = run_loop("G#&>0,3,1,0,C,2,1,x:");

    assert_eq!(commands.len(), 4);

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::ColorSet { pen, color } => {
                assert_eq!(*pen as u8, 1);
                assert_eq!(*color, i as u8);
            }
            _ => panic!("Expected ColorSet command"),
        }
    }
}

#[test]
fn test_run_loop_multiple_iterations() {
    // Loop with many iterations
    let commands = run_loop("G#&>0,10,2,0,O,3,100,100,x:");

    // from=0, to=10, step=2
    // iterations: 0, 2, 4, 6, 8, 10
    assert_eq!(commands.len(), 6);

    let expected = [0, 2, 4, 6, 8, 10];

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Circle { x, y, radius } => {
                assert_eq!(*x, 100.into());
                assert_eq!(*y, 100.into());
                assert_eq!(*radius, expected[i].into());
            }
            _ => panic!("Expected Circle command"),
        }
    }
}

#[test]
fn test_run_loop_mixed_parameters() {
    // Complex loop mixing constants, step vars, and expressions
    let commands = run_loop("G#&>0,2,1,0,L,4,10,x,+100,y:");

    assert_eq!(commands.len(), 3);

    // x goes 0->2, y goes 2->0, +100 is x+100
    let expected = [(10, 0, 100, 2), (10, 1, 101, 1), (10, 2, 102, 0)];

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Line { x1, y1, x2, y2 } => {
                assert_eq!(*x1, expected[i].0.into());
                assert_eq!(*y1, expected[i].1.into());
                assert_eq!(*x2, expected[i].2.into());
                assert_eq!(*y2, expected[i].3.into());
            }
            _ => panic!("Expected Line command"),
        }
    }
}

#[test]
fn test_run_loop_zero_step_no_iterations() {
    // Loop with step=0 should not execute
    let commands = run_loop("G#&>0,10,0,0,L,4,0,0,100,100:");

    // Should generate no commands
    assert_eq!(commands.len(), 0);
}

#[test]
fn test_run_loop_single_iteration() {
    // Loop with from==to should execute once
    let commands = run_loop("G#&>5,5,1,0,O,3,50,50,x:");

    assert_eq!(commands.len(), 1);

    match &commands[0] {
        IgsCommand::Circle { x, y, radius } => {
            assert_eq!(*x, 50.into());
            assert_eq!(*y, 50.into());
            assert_eq!(*radius, 5.into());
        }
        _ => panic!("Expected Circle command"),
    }
}

#[test]
fn test_run_loop_random_placeholder() {
    // Loop with 'r' (random) - should generate random values in the parameter bounds
    let commands = run_loop("G#&>0,2,1,0,O,3,50,50,r:");

    assert_eq!(commands.len(), 3);

    // Default bounds are (0,199), so 'r' should generate values in this range
    for cmd in &commands {
        match cmd {
            IgsCommand::Circle { x, y, radius } => {
                assert_eq!(*x, 50.into());
                assert_eq!(*y, 50.into());
                // Radius should be a random value within bounds (0-199)
                if let IgsParameter::Value(r) = radius {
                    assert!(*r >= 0 && *r <= 199, "Random radius {} should be in range [0, 199]", r);
                } else {
                    panic!("Expected Value parameter for radius");
                }
            }
            _ => panic!("Expected Circle command"),
        }
    }
}

#[test]
fn test_run_loop_arc_command() {
    // Loop drawing arcs (K command, not A)
    let commands = run_loop("G#&>0,2,1,0,K,5,50,50,x,0,90:");

    assert_eq!(commands.len(), 3);

    for (i, cmd) in commands.iter().enumerate() {
        match cmd {
            IgsCommand::Arc {
                x,
                y,
                radius,
                start_angle,
                end_angle,
            } => {
                assert_eq!(*x, 50.into());
                assert_eq!(*y, 50.into());
                assert_eq!(*radius, (i as i32).into());
                assert_eq!(*start_angle, 0.into());
                assert_eq!(*end_angle, 90.into());
            }
            _ => panic!("Expected Arc command"),
        }
    }
}

#[test]
fn test_run_loop_with_large_step() {
    // Loop with step larger than range
    let commands = run_loop("G#&>0,5,10,0,O,3,50,50,x:");

    // from=0, to=5, step=10
    // Only iteration 0 should execute (0 <= 5, then 0+10=10 > 5)
    assert_eq!(commands.len(), 1);

    match &commands[0] {
        IgsCommand::Circle { x, y, radius } => {
            assert_eq!(*x, 50.into());
            assert_eq!(*y, 50.into());
            assert_eq!(*radius, 0.into());
        }
        _ => panic!("Expected Circle command"),
    }
}
