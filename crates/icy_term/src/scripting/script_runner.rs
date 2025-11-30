//! Script runner for executing Lua scripts in the terminal context
//!
//! Handles loading, executing, and managing Lua scripts with terminal access.

use std::path::Path;
use std::sync::Arc;
use std::thread;

use icy_engine::Screen;
use icy_engine_scripting::LuaExtension;
use icy_net::telnet::TerminalEmulation;
use mlua::Lua;
use parking_lot::Mutex;
use regex::Regex;
use tokio::sync::mpsc;

use super::terminal_extension::{ScriptState, TerminalLuaExtension};
use crate::data::AddressBook;
use crate::terminal::{TerminalCommand, TerminalEvent};

lazy_static::lazy_static! {
    static ref HEX_REGEX: Regex = Regex::new(r"#([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})").unwrap();
}

/// Result of script execution
#[derive(Debug)]
pub enum ScriptResult {
    Success,
    Error(String),
    Stopped,
}

/// Runs Lua scripts with terminal access
pub struct ScriptRunner {
    /// Shared state with terminal
    state: Arc<ScriptState>,
    /// Running script thread handle
    run_thread: Option<thread::JoinHandle<ScriptResult>>,
}

impl ScriptRunner {
    /// Create a new script runner
    pub fn new(
        screen: Arc<Mutex<Box<dyn Screen>>>,
        command_tx: mpsc::UnboundedSender<TerminalCommand>,
        event_tx: mpsc::UnboundedSender<TerminalEvent>,
        address_book: Arc<Mutex<AddressBook>>,
        terminal_emulation: Arc<Mutex<TerminalEmulation>>,
    ) -> Self {
        let state = Arc::new(ScriptState::new(screen, command_tx, event_tx, address_book, terminal_emulation));
        Self { state, run_thread: None }
    }

    /// Run a script from a file
    pub fn run_file(&mut self, path: &Path) -> Result<(), String> {
        let script =
            std::fs::read_to_string(path).map_err(|e| format!("{}:\n{}", i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "error-script-file-read-failed"), e))?;

        self.run_script(script)
    }

    /// Run a script from a string
    pub fn run_script(&mut self, script: String) -> Result<(), String> {
        // Stop any currently running script
        self.stop();

        // Reset state
        *self.state.should_stop.lock() = false;

        let state = self.state.clone();

        let handle = thread::spawn(move || run_lua_script(state, script));

        self.run_thread = Some(handle);
        Ok(())
    }

    /// Check if a script is currently running
    pub fn is_running(&self) -> bool {
        if let Some(handle) = &self.run_thread { !handle.is_finished() } else { false }
    }

    /// Stop the currently running script
    pub fn stop(&mut self) {
        *self.state.should_stop.lock() = true;

        if let Some(handle) = self.run_thread.take() {
            // Wait for thread to finish (with timeout)
            let _ = handle.join();
        }
    }

    /// Get the result if script has finished
    pub fn get_result(&mut self) -> Option<ScriptResult> {
        if let Some(handle) = &self.run_thread {
            if handle.is_finished() {
                if let Some(handle) = self.run_thread.take() {
                    return handle.join().ok();
                }
            }
        }
        None
    }
}

fn run_lua_script(state: Arc<ScriptState>, script: String) -> ScriptResult {
    let lua = Lua::new();
    let globals = lua.globals();

    // Preprocess hex colors (#RRGGBB -> r,g,b)
    let script = HEX_REGEX
        .replace_all(&script, |caps: &regex::Captures<'_>| {
            let r = u32::from_str_radix(caps.get(1).unwrap().as_str(), 16).unwrap();
            let g = u32::from_str_radix(caps.get(2).unwrap().as_str(), 16).unwrap();
            let b = u32::from_str_radix(caps.get(3).unwrap().as_str(), 16).unwrap();
            format!("{},{},{}", r, g, b)
        })
        .to_string();

    // Register terminal extension functions
    let extension = TerminalLuaExtension::new(state.clone());
    if let Err(e) = extension.register(&lua) {
        return ScriptResult::Error(format!("Failed to register extension: {}", e));
    }

    // Register log function
    if let Err(e) = globals.set(
        "log",
        lua.create_function(move |_, msg: String| {
            log::info!("{}", msg);
            Ok(())
        })
        .unwrap(),
    ) {
        return ScriptResult::Error(format!("Failed to register log function: {}", e));
    }

    // Register print override to also log
    if let Err(e) = globals.set(
        "print",
        lua.create_function(move |_, args: mlua::Variadic<String>| {
            let msg = args.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\t");
            log::info!("{}", msg);
            Ok(())
        })
        .unwrap(),
    ) {
        return ScriptResult::Error(format!("Failed to register print function: {}", e));
    }

    // Execute the script
    match lua.load(script).exec() {
        Ok(()) => {
            if *state.should_stop.lock() {
                ScriptResult::Stopped
            } else {
                ScriptResult::Success
            }
        }
        Err(e) => ScriptResult::Error(format!("{}", e)),
    }
}

impl Drop for ScriptRunner {
    fn drop(&mut self) {
        self.stop();
    }
}
