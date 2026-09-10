use icy_engine::{formats::FileFormat, AttributedChar, Position, TextAttribute, TextPane, TextScreen};
use icy_engine_gui::version_helper::replace_version_marker;

const SCREENS: [&[u8]; 2] = [include_bytes!("../data/welcome_screen.1.icy"), include_bytes!("../data/welcome_screen.2.icy")];
const READY_MESSAGES: [&str; 6] = ["READY.", "STANDING BY...", "OK", "AWAITING INPUT", "SESSION STARTED.", "SYSTEM READY"];

pub fn create_welcome_screen(mcp_port: Option<u16>) -> TextScreen {
    build_screen(
        SCREENS[fastrand::usize(0..SCREENS.len())],
        READY_MESSAGES[fastrand::usize(0..READY_MESSAGES.len())],
        mcp_port,
    )
}

fn build_screen(bytes: &[u8], ready_message: &str, mcp_port: Option<u16>) -> TextScreen {
    let mut screen = FileFormat::IcyDraw.from_bytes(bytes, None).expect("bundled welcome screen").screen;
    screen.buffer.buffer_type = icy_engine::BufferType::CP437;
    screen.buffer.terminal_state.is_terminal_buffer = true;
    let version = semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("package version");
    let mut position = replace_version_marker(&mut screen.buffer, &version, None)
        .map(|(column, row)| Position::new(column, row))
        .unwrap_or_default();
    if let Some(port) = mcp_port.filter(|port| *port != 0) {
        write_line(
            &mut screen,
            &mut position,
            &format!("MCP SERVER STARTED ON PORT {port}."),
            TextAttribute::from_u8(0x0E, icy_engine::IceMode::Ice),
        );
    }
    write_line(&mut screen, &mut position, ready_message, TextAttribute::default());
    screen.buffer.update_hyperlinks();
    screen.caret.set_position(position);
    screen
}

fn write_line(screen: &mut TextScreen, position: &mut Position, text: &str, attribute: TextAttribute) {
    for character in text.chars() {
        if position.x < screen.buffer.width() {
            screen.buffer.layers[0].set_char(*position, AttributedChar::new(character, attribute));
            position.x += 1;
        }
    }
    *position = Position::new(0, position.y + 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(screen: &TextScreen, row: i32) -> String {
        (0..screen.width())
            .map(|column| screen.buffer.layers[0].char_at(Position::new(column, row)).ch)
            .collect()
    }

    #[test]
    fn both_variants_place_ready_and_cursor_below_the_artwork() {
        for bytes in SCREENS {
            let screen = build_screen(bytes, "READY.", None);
            let position = screen.caret.position();
            assert_eq!(position.x, 0);
            assert!(position.y > 1 && position.y < screen.height(), "{position:?}");
            assert!(line(&screen, position.y - 1).starts_with("READY."));
            assert!(line(&screen, position.y).trim().is_empty());
            assert_eq!(screen.buffer.buffer_type, icy_engine::BufferType::CP437);
            assert!(screen.buffer.terminal_state.is_terminal_buffer);
        }
    }

    #[test]
    fn mcp_notice_precedes_ready_without_changing_cursor_column() {
        for bytes in SCREENS {
            let screen = build_screen(bytes, "READY.", Some(9000));
            let position = screen.caret.position();
            assert_eq!(position.x, 0);
            assert!(line(&screen, position.y - 2).starts_with("MCP SERVER STARTED ON PORT 9000."));
            assert!(line(&screen, position.y - 1).starts_with("READY."));
        }
    }
}
