pub fn create_welcome_screen() -> icy_engine::TextScreen {
    let port = crate::MCP_PORT.swap(0, std::sync::atomic::Ordering::Relaxed);
    icy_term::welcome_screen::create_welcome_screen((port != 0).then_some(port))
}
