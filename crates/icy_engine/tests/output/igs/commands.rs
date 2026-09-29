//! IGS commands checked pixel by pixel against how VDI and IG draw them.

use icy_engine::{Color, EditableScreen, ScreenMode, ScreenSink, TerminalResolution};
use icy_net::telnet::TerminalEmulation;

fn run(resolution: TerminalResolution, source: &[u8]) -> Box<dyn EditableScreen> {
    let (mut screen, mut parser) = ScreenMode::AtariST(resolution, true).create_screen(TerminalEmulation::AtariST, None);
    let mut sink = ScreenSink::new(&mut *screen);
    parser.parse(source, &mut sink);
    screen
}

fn pixel(screen: &dyn EditableScreen, x: i32, y: i32) -> u8 {
    screen.screen()[(y * screen.resolution().width + x) as usize]
}

fn column(screen: &dyn EditableScreen, x: i32, rows: std::ops::Range<i32>) -> Vec<bool> {
    rows.map(|y| pixel(screen, x, y) != 0).collect()
}

#[test]
fn wide_lines_use_the_pen_height_of_the_pixel_aspect() {
    // Width 9 covers 9 rows on the low resolution's almost square pixels.
    let screen = run(TerminalResolution::Low, b"G#s>4:T>2,1,9:L>20,50,100,50:");
    assert!((46..=54).all(|y| pixel(&*screen, 60, y) != 0), "{:?}", column(&*screen, 60, 40..60));
    assert!(
        pixel(&*screen, 60, 45) == 0 && pixel(&*screen, 60, 55) == 0,
        "{:?}",
        column(&*screen, 60, 40..60)
    );
    // Square ends stop at the end points.
    assert_eq!(pixel(&*screen, 19, 50), 0);
    assert_ne!(pixel(&*screen, 20, 50), 0);

    // Even widths are drawn one narrower, and medium resolution pixels are twice as tall.
    let screen = run(TerminalResolution::Medium, b"G#s>4:T>2,1,8:L>20,50,100,50:");
    assert!((49..=51).all(|y| pixel(&*screen, 60, y) != 0) && pixel(&*screen, 60, 48) == 0 && pixel(&*screen, 60, 52) == 0);
}

#[test]
fn rounded_and_arrow_ends_extend_and_point_the_line() {
    let rounded = run(TerminalResolution::Low, b"G#s>4:T>2,1,9:T>2,1,60:L>20,50,100,50:");
    assert_ne!(pixel(&*rounded, 16, 50), 0, "a rounded end reaches past the end point");
    assert_ne!(pixel(&*rounded, 104, 50), 0);

    // An arrow on the right of a thin line: the head is wider than the line near the tip.
    let arrow = run(TerminalResolution::Low, b"G#s>4:T>2,1,52:L>20,50,100,50:");
    assert_ne!(pixel(&*arrow, 100, 50), 0);
    assert_ne!(pixel(&*arrow, 94, 52), 0);
    assert_ne!(pixel(&*arrow, 94, 48), 0);
    assert_eq!(pixel(&*arrow, 60, 52), 0);
    assert_eq!(pixel(&*arrow, 21, 52), 0, "no head on the square start");
}

#[test]
fn spray_paint_stays_in_its_area_and_rotates_colors() {
    let screen = run(TerminalResolution::Low, b"G#s>4:X>0,50,60,40,20,300:");
    let res = screen.resolution();
    let mut inside = 0;
    for y in 0..res.height {
        for x in 0..res.width {
            if pixel(&*screen, x, y) != 0 {
                assert!((50..=90).contains(&x) && (60..=80).contains(&y), "sprayed outside at {x},{y}");
                inside += 1;
            }
        }
    }
    assert!(inside > 100, "{inside} points sprayed");

    let rotating = run(TerminalResolution::Low, b"G#s>4:X>0,1,0,0,0,0:X>0,50,60,40,20,300:");
    let colors: std::collections::HashSet<u8> = (60..=80)
        .flat_map(|y| (50..=90).map(move |x| (x, y)))
        .map(|(x, y)| pixel(&*rotating, x, y))
        .collect();
    assert!(colors.len() > 8, "sprayed colors {colors:?}");

    // Rotation is off again with pen 0.
    let off = run(TerminalResolution::Low, b"G#s>4:X>0,1,0,0,0,0:X>0,0,0,0,0,0:X>0,50,60,40,20,300:");
    let colors: std::collections::HashSet<u8> = (60..=80)
        .flat_map(|y| (50..=90).map(move |x| (x, y)))
        .map(|(x, y)| pixel(&*off, x, y))
        .collect();
    assert_eq!(colors.len(), 2, "background and marker color: {colors:?}");
}

#[test]
fn color_registers_are_set_loaded_rotated_and_restored() {
    let red = Color::new(238, 0, 0);
    let green = Color::new(0, 238, 0);
    let blue = Color::new(0, 0, 238);
    let screen = run(TerminalResolution::Low, b"G#X>1,1,1792:X>1,2,112:X>1,3,7:");
    assert_eq!(
        [screen.palette().color(1), screen.palette().color(2), screen.palette().color(3)],
        [red.clone(), green.clone(), blue.clone()]
    );

    // Shifting right moves the last color of the range to the first register.
    let rotated = run(TerminalResolution::Low, b"G#X>1,1,1792:X>1,2,112:X>1,3,7:X>8,1,3,1,0:");
    assert_eq!(
        [rotated.palette().color(1), rotated.palette().color(2), rotated.palette().color(3)],
        [blue.clone(), red.clone(), green.clone()]
    );
    let left = run(TerminalResolution::Low, b"G#X>1,1,1792:X>1,2,112:X>1,3,7:X>8,3,1,1,0:");
    assert_eq!(
        [left.palette().color(1), left.palette().color(2), left.palette().color(3)],
        [green.clone(), blue.clone(), red.clone()]
    );
    let restored = run(TerminalResolution::Low, b"G#X>1,1,1792:X>1,2,112:X>1,3,7:X>8,1,3,2,0:X>8,1,1,1,1:");
    assert_eq!(
        [restored.palette().color(1), restored.palette().color(2), restored.palette().color(3)],
        [red, green, blue]
    );

    // X 12 loads four registers at a time; STE values use bit 3 as the lowest bit.
    let loaded = run(TerminalResolution::Low, b"G#X>12,1,1911,0,15,8:");
    assert_eq!(loaded.palette().color(4), Color::new(238, 238, 238));
    assert_eq!(loaded.palette().color(5), Color::new(0, 0, 0));
    assert_eq!(loaded.palette().color(6), Color::new(0, 0, 255));
    assert_eq!(loaded.palette().color(7), Color::new(0, 0, 17));
}

#[test]
fn wiped_blit_memory_shows_the_byte_as_planar_pixels() {
    // 0xF0 in every byte: the first four pixels of each byte use all planes.
    let screen = run(TerminalResolution::Low, b"G#s>4:X>11,0,0,240:G>2,3,0,0:");
    assert_eq!((0..8).map(|x| pixel(&*screen, x, 10)).collect::<Vec<_>>(), [15, 15, 15, 15, 0, 0, 0, 0]);
}
