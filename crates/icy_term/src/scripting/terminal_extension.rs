//! Terminal-specific Lua extension
//!
//! Provides terminal operations like send, wait_for, sleep, connect, etc.

use std::sync::Arc;
use std::time::Duration;

use iced::keyboard;
use icy_engine::Screen;
use icy_engine_scripting::{LuaExtension, LuaScreen};
use icy_net::telnet::TerminalEmulation;
use mlua::Lua;
use parking_lot::Mutex;
use regex::Regex;
use tokio::sync::mpsc;

use crate::data::AddressBook;
use crate::terminal::{TerminalCommand, TerminalEvent};

/// Shared state between Lua script and terminal thread
pub struct ScriptState {
    /// The terminal screen buffer
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    /// Channel to send commands to terminal
    pub command_tx: mpsc::UnboundedSender<TerminalCommand>,
    /// Channel to send events to UI
    pub event_tx: mpsc::UnboundedSender<TerminalEvent>,
    /// Flag to signal script should stop
    pub should_stop: Arc<Mutex<bool>>,
    /// Address book for BBS lookups (kept for potential future use)
    #[allow(dead_code)]
    pub address_book: Arc<Mutex<AddressBook>>,
    /// Current terminal emulation type for key mapping
    pub terminal_emulation: Arc<Mutex<TerminalEmulation>>,
}

impl ScriptState {
    pub fn new(
        screen: Arc<Mutex<Box<dyn Screen>>>,
        command_tx: mpsc::UnboundedSender<TerminalCommand>,
        event_tx: mpsc::UnboundedSender<TerminalEvent>,
        address_book: Arc<Mutex<AddressBook>>,
        terminal_emulation: Arc<Mutex<TerminalEmulation>>,
    ) -> Self {
        Self {
            screen,
            command_tx,
            event_tx,
            should_stop: Arc::new(Mutex::new(false)),
            address_book,
            terminal_emulation,
        }
    }
}

/// Lua extension for terminal-specific functions
pub struct TerminalLuaExtension {
    state: Arc<ScriptState>,
}

impl TerminalLuaExtension {
    pub fn new(state: Arc<ScriptState>) -> Self {
        Self { state }
    }
}

/// Extracts the current screen content as a String
fn get_screen_text(screen: &Arc<Mutex<Box<dyn Screen>>>) -> String {
    let screen_guard = screen.lock();
    let width = screen_guard.get_width();
    let height = screen_guard.get_height();
    let buffer_type = screen_guard.buffer_type();

    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            let ch = screen_guard.get_char(icy_engine::Position::new(x, y));
            text.push(buffer_type.convert_to_unicode(ch.ch));
        }
        text.push('\n');
    }
    text
}

impl LuaExtension for TerminalLuaExtension {
    fn register(&self, lua: &Lua) -> mlua::Result<()> {
        let globals = lua.globals();

        // Register send() function
        self.register_send(lua, &globals)?;

        // Register sleep() function
        self.register_sleep(lua, &globals)?;

        // Register wait_for() function
        self.register_wait_for(lua, &globals)?;

        // Register on_screen() function
        self.register_on_screen(lua, &globals)?;

        // Register get_screen() function
        self.register_get_screen(lua, &globals)?;

        // Register is_connected() function
        self.register_is_connected(lua, &globals)?;

        // Register disconnect() function
        self.register_disconnect(lua, &globals)?;

        // Register connect() function
        self.register_connect(lua, &globals)?;

        // Register send_login(), send_username(), send_password() functions
        self.register_send_login(lua, &globals)?;
        self.register_send_username(lua, &globals)?;
        self.register_send_password(lua, &globals)?;

        // Register send_key() function
        self.register_send_key(lua, &globals)?;

        // Register quit() function
        self.register_quit(lua, &globals)?;

        Ok(())
    }
}

impl TerminalLuaExtension {
    fn register_send(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let command_tx = self.state.command_tx.clone();
        globals.set(
            "send",
            lua.create_function(move |_, text: String| {
                let data = text.into_bytes();
                if command_tx.send(TerminalCommand::SendData(data)).is_err() {
                    return Err(mlua::Error::RuntimeError("Failed to send data".to_string()));
                }
                Ok(())
            })?,
        )?;
        Ok(())
    }

    fn register_sleep(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        globals.set(
            "sleep",
            lua.create_function(move |_, ms: u64| {
                std::thread::sleep(Duration::from_millis(ms));
                Ok(())
            })?,
        )?;
        Ok(())
    }

    fn register_wait_for(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let screen = self.state.screen.clone();
        let should_stop = self.state.should_stop.clone();

        globals.set(
            "wait_for",
            lua.create_function(move |_, (pattern, timeout_ms): (String, Option<u64>)| {
                let timeout = timeout_ms.unwrap_or(30000); // Default 30 seconds
                let start = std::time::Instant::now();

                let regex = Regex::new(&pattern).map_err(|e| mlua::Error::RuntimeError(format!("Invalid regex pattern: {}", e)))?;

                loop {
                    // Check if we should stop
                    if *should_stop.lock() {
                        return Err(mlua::Error::RuntimeError("Script stopped".to_string()));
                    }

                    // Check timeout
                    if start.elapsed().as_millis() as u64 > timeout {
                        return Ok(None);
                    }

                    // Get screen content as text
                    let screen_text = get_screen_text(&screen);

                    // Search for pattern in screen text
                    if let Some(m) = regex.find(&screen_text) {
                        return Ok(Some(m.as_str().to_string()));
                    }

                    // Small sleep to avoid busy-waiting
                    std::thread::sleep(Duration::from_millis(50));
                }
            })?,
        )?;
        Ok(())
    }

    fn register_on_screen(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let screen = self.state.screen.clone();

        globals.set(
            "on_screen",
            lua.create_function(move |_, pattern: String| {
                // Get screen content as text
                let screen_text = get_screen_text(&screen);

                // Check if pattern exists in screen text (supports regex)
                let regex = Regex::new(&pattern).map_err(|e| mlua::Error::RuntimeError(format!("Invalid regex pattern: {}", e)))?;

                Ok(regex.is_match(&screen_text))
            })?,
        )?;
        Ok(())
    }

    fn register_get_screen(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let screen = self.state.screen.clone();

        // Register screen as global LuaScreen object
        let lua_screen = LuaScreen::new(screen);
        globals.set("screen", lua_screen)?;

        // Load global wrapper functions from external Lua file
        lua.load(include_str!("screen_api.lua")).exec()?;

        Ok(())
    }

    fn register_is_connected(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        // For now, we assume connected if we can send commands
        let command_tx = self.state.command_tx.clone();

        globals.set(
            "is_connected",
            lua.create_function(move |_, ()| {
                let connected = !command_tx.is_closed();
                Ok(connected)
            })?,
        )?;
        Ok(())
    }

    fn register_disconnect(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let command_tx = self.state.command_tx.clone();

        globals.set(
            "disconnect",
            lua.create_function(move |_, ()| {
                if command_tx.send(TerminalCommand::Disconnect).is_err() {
                    return Err(mlua::Error::RuntimeError("Failed to disconnect".to_string()));
                }
                Ok(())
            })?,
        )?;
        Ok(())
    }

    fn register_connect(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let event_tx = self.state.event_tx.clone();

        globals.set(
            "connect",
            lua.create_function(move |_, name_or_url: String| {
                // Send connect request to UI (MainWindow handles address book lookup)
                if event_tx.send(TerminalEvent::Connect(name_or_url.clone())).is_err() {
                    return Err(mlua::Error::RuntimeError("Failed to send connect request".to_string()));
                }

                // Give the UI time to process the connect request and establish connection
                // The script should use wait_for() to wait for actual connection prompts
                std::thread::sleep(std::time::Duration::from_millis(1000));

                Ok(name_or_url)
            })?,
        )?;
        Ok(())
    }

    fn register_send_login(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let event_tx = self.state.event_tx.clone();

        globals.set(
            "send_login",
            lua.create_function(move |_, ()| {
                // Send username + password with delay
                if event_tx.send(TerminalEvent::SendCredentials(0)).is_err() {
                    return Err(mlua::Error::RuntimeError("Failed to send login credentials".to_string()));
                }

                // Wait for credentials to be sent (username + 500ms delay + password)
                std::thread::sleep(std::time::Duration::from_millis(600));

                Ok(())
            })?,
        )?;
        Ok(())
    }

    fn register_send_username(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let event_tx = self.state.event_tx.clone();

        globals.set(
            "send_username",
            lua.create_function(move |_, ()| {
                // Send username only
                if event_tx.send(TerminalEvent::SendCredentials(1)).is_err() {
                    return Err(mlua::Error::RuntimeError("Failed to send username".to_string()));
                }

                std::thread::sleep(std::time::Duration::from_millis(100));

                Ok(())
            })?,
        )?;
        Ok(())
    }

    fn register_send_password(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let event_tx = self.state.event_tx.clone();

        globals.set(
            "send_password",
            lua.create_function(move |_, ()| {
                // Send password only
                if event_tx.send(TerminalEvent::SendCredentials(2)).is_err() {
                    return Err(mlua::Error::RuntimeError("Failed to send password".to_string()));
                }

                std::thread::sleep(std::time::Duration::from_millis(100));

                Ok(())
            })?,
        )?;
        Ok(())
    }

    fn register_send_key(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let command_tx = self.state.command_tx.clone();
        let terminal_emulation = self.state.terminal_emulation.clone();

        globals.set(
            "send_key",
            lua.create_function(move |_, key_str: String| {
                let term_emu = *terminal_emulation.lock();
                let bytes = parse_key_string(term_emu, &key_str);

                match bytes {
                    Some(data) => {
                        if command_tx.send(TerminalCommand::SendData(data)).is_err() {
                            return Err(mlua::Error::RuntimeError("Failed to send key".to_string()));
                        }
                        Ok(true)
                    }
                    None => Ok(false),
                }
            })?,
        )?;
        Ok(())
    }

    fn register_quit(&self, lua: &Lua, globals: &mlua::Table) -> mlua::Result<()> {
        let event_tx = self.state.event_tx.clone();

        globals.set(
            "quit",
            lua.create_function(move |_, ()| {
                if event_tx.send(TerminalEvent::Quit).is_err() {
                    return Err(mlua::Error::RuntimeError("Failed to send quit request".to_string()));
                }
                Ok(())
            })?,
        )?;
        Ok(())
    }
}

/// Parse a key string (like "enter", "left", "f1") to bytes for the given terminal emulation
pub fn parse_key_string(terminal_type: TerminalEmulation, key_str: &str) -> Option<Vec<u8>> {
    let key = match key_str.to_lowercase().as_str() {
        "enter" | "return" => keyboard::Key::Named(keyboard::key::Named::Enter),
        "escape" | "esc" => keyboard::Key::Named(keyboard::key::Named::Escape),
        "tab" => keyboard::Key::Named(keyboard::key::Named::Tab),
        "backspace" => keyboard::Key::Named(keyboard::key::Named::Backspace),
        "delete" | "del" => keyboard::Key::Named(keyboard::key::Named::Delete),
        "home" => keyboard::Key::Named(keyboard::key::Named::Home),
        "end" => keyboard::Key::Named(keyboard::key::Named::End),
        "pageup" | "pgup" => keyboard::Key::Named(keyboard::key::Named::PageUp),
        "pagedown" | "pgdn" => keyboard::Key::Named(keyboard::key::Named::PageDown),
        "up" | "arrowup" => keyboard::Key::Named(keyboard::key::Named::ArrowUp),
        "down" | "arrowdown" => keyboard::Key::Named(keyboard::key::Named::ArrowDown),
        "left" | "arrowleft" => keyboard::Key::Named(keyboard::key::Named::ArrowLeft),
        "right" | "arrowright" => keyboard::Key::Named(keyboard::key::Named::ArrowRight),
        "f1" => keyboard::Key::Named(keyboard::key::Named::F1),
        "f2" => keyboard::Key::Named(keyboard::key::Named::F2),
        "f3" => keyboard::Key::Named(keyboard::key::Named::F3),
        "f4" => keyboard::Key::Named(keyboard::key::Named::F4),
        "f5" => keyboard::Key::Named(keyboard::key::Named::F5),
        "f6" => keyboard::Key::Named(keyboard::key::Named::F6),
        "f7" => keyboard::Key::Named(keyboard::key::Named::F7),
        "f8" => keyboard::Key::Named(keyboard::key::Named::F8),
        "f9" => keyboard::Key::Named(keyboard::key::Named::F9),
        "f10" => keyboard::Key::Named(keyboard::key::Named::F10),
        "f11" => keyboard::Key::Named(keyboard::key::Named::F11),
        "f12" => keyboard::Key::Named(keyboard::key::Named::F12),
        _ => return None,
    };

    let modifiers = keyboard::Modifiers::empty();
    let physical = keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified);

    map_key_to_bytes(terminal_type, &key, &physical, modifiers)
}

/// Map a keyboard key event to bytes for the given terminal emulation
fn map_key_to_bytes(
    terminal_type: TerminalEmulation,
    key: &keyboard::Key,
    physical: &keyboard::key::Physical,
    modifiers: keyboard::Modifiers,
) -> Option<Vec<u8>> {
    let key_map = match terminal_type {
        TerminalEmulation::PETscii => icy_engine_gui::key_map::C64_KEY_MAP,
        TerminalEmulation::ViewData => icy_engine_gui::key_map::VIDEOTERM_KEY_MAP,
        TerminalEmulation::Mode7 => icy_engine_gui::key_map::MODE7_KEY_MAP,
        TerminalEmulation::ATAscii => icy_engine_gui::key_map::ATASCII_KEY_MAP,
        TerminalEmulation::AtariST => icy_engine_gui::key_map::ATARI_ST_KEY_MAP,
        _ => icy_engine_gui::key_map::ANSI_KEY_MAP,
    };

    icy_engine_gui::key_map::lookup_key(key, physical, modifiers, key_map)
}
