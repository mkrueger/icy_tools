use crate::{MCP_PORT, VERSION};
use icy_engine::{AttributedChar, Position, TextAttribute, TextPane, TextScreen, formats::FileFormat};
use icy_engine_gui::version_helper::replace_version_marker;

const MAIN_SCREEN_ANSI1: &[u8] = include_bytes!("../../data/welcome_screen.1.icy");
const MAIN_SCREEN_ANSI2: &[u8] = include_bytes!("../../data/welcome_screen.2.icy");

pub fn create_welcome_screen() -> TextScreen {
    // Load the welcome screen from MAIN_SCREEN_ANSI
    let mut screen = FileFormat::IcyDraw
        .from_bytes(if fastrand::bool() { MAIN_SCREEN_ANSI1 } else { MAIN_SCREEN_ANSI2 }, None)
        .unwrap()
        .screen;
    screen.buffer.buffer_type = icy_engine::BufferType::CP437;
    screen.buffer.terminal_state.is_terminal_buffer = true;
    // Find and replace special characters
    let ready_position = replace_version_marker(&mut screen.buffer, &VERSION, None);

    // Write "IcyTerm ready." message at the marked position
    let mut caret_pos = Position::default();
    if let Some((x, y)) = ready_position {
        caret_pos = Position::new(x, y);
    }

    // Check if MCP port is set and print message
    let port = MCP_PORT.load(std::sync::atomic::Ordering::Relaxed);
    if port != 0 {
        // Print MCP message in yellow (color 14)
        let mcp_msg = format!("MCP SERVER STARTED ON PORT {}.", port);
        let yellow_attr = TextAttribute::from_u8(0x0E, icy_engine::IceMode::Ice);

        for msg_char in mcp_msg.chars() {
            if caret_pos.x < screen.buffer.width() {
                let new_ch = AttributedChar::new(msg_char, yellow_attr);
                screen.buffer.layers[0].set_char(Position::new(caret_pos.x, caret_pos.y), new_ch);
                caret_pos.x += 1;
            }
        }

        // Set cursor position after the ready message
        caret_pos = Position::new(0, caret_pos.y + 1);

        // Reset MCP_PORT to 0
        MCP_PORT.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    let variants = ["READY.", "STANDING BY...", "OK", "AWAITING INPUT", "SESSION STARTED.", "SYSTEM READY"];
    let ready_msg = variants[fastrand::usize(0..variants.len())];

    for msg_char in ready_msg.chars() {
        if caret_pos.x < screen.buffer.width() {
            let new_ch = AttributedChar::new(msg_char, TextAttribute::default());
            screen.buffer.layers[0].set_char(Position::new(caret_pos.x, caret_pos.y), new_ch);
            caret_pos.x += 1;
        }
    }

    // Set cursor position after the ready message
    caret_pos = Position::new(0, caret_pos.y + 1);

    screen.buffer.update_hyperlinks();
    screen.caret.set_position(caret_pos);

    screen
}
