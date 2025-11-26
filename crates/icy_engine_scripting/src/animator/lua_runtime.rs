//! Lua runtime setup and script execution

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use icy_engine::{Screen, TextBuffer, TextPane, TextScreen};
use mlua::{Lua, Value};
use parking_lot::Mutex;
use regex::Regex;

use super::lua_layer::LuaLayer;
use super::{Animator, LogEntry};
use crate::MonitorType;

lazy_static::lazy_static! {
    static ref HEX_REGEX: Regex = Regex::new(r"#([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})").unwrap();
}

impl Animator {
    /// Run an animation script in a background thread
    pub fn run(parent: &Option<PathBuf>, in_txt: String) -> Arc<Mutex<Self>> {
        let animator = Arc::new(Mutex::new(Animator::default()));
        let animator_thread = animator.clone();
        let parent = parent.clone();

        let run_thread = thread::spawn(move || {
            run_lua_script(&animator_thread, &parent, in_txt);
        });

        animator.lock().run_thread = Some(run_thread);
        animator
    }
}

fn run_lua_script(animator: &Arc<Mutex<Animator>>, parent: &Option<PathBuf>, in_txt: String) {
    let lua: Lua = Lua::new();
    let globals = lua.globals();

    // Preprocess hex colors (#RRGGBB -> r,g,b)
    let txt = HEX_REGEX
        .replace_all(&in_txt, |caps: &regex::Captures<'_>| {
            let r = u32::from_str_radix(caps.get(1).unwrap().as_str(), 16).unwrap();
            let g = u32::from_str_radix(caps.get(2).unwrap().as_str(), 16).unwrap();
            let b = u32::from_str_radix(caps.get(3).unwrap().as_str(), 16).unwrap();
            format!("{},{},{}", r, g, b)
        })
        .to_string();

    // Register load_buffer function
    register_load_buffer(&lua, &globals, parent);

    // Register new_buffer function
    register_new_buffer(&lua, &globals);

    // Register next_frame function
    register_next_frame(&lua, &globals, animator);

    // Register delay functions
    register_delay_functions(&lua, &globals, animator);

    // Register log function
    register_log_function(&lua, &globals, animator);

    // Initialize global variables
    initialize_globals(&globals, animator);

    // Execute the script
    if let Err(err) = lua.load(txt).exec() {
        animator.lock().error = format!("{err}");
    }
}

fn register_load_buffer(lua: &Lua, globals: &mlua::Table, parent: &Option<PathBuf>) {
    let parent = parent.clone();
    globals
        .set(
            "load_buffer",
            lua.create_function(move |_lua, file: String| {
                let mut file_name = Path::new(&file).to_path_buf();
                if file_name.is_relative() {
                    if let Some(parent) = &parent {
                        file_name = parent.join(&file_name);
                    }
                }

                if !file_name.exists() {
                    return Err(mlua::Error::RuntimeError(format!("File not found {}", file)));
                }

                if let Ok(buffer) = TextBuffer::load_buffer(&file_name, true, None) {
                    let mut text_screen = TextScreen::new(buffer.get_size());
                    text_screen.buffer = buffer;
                    let screen: Box<dyn Screen> = Box::new(text_screen);
                    mlua::Result::Ok(LuaLayer::new(Arc::new(Mutex::new(screen))))
                } else {
                    Err(mlua::Error::RuntimeError(format!("Could not load file {}", file)))
                }
            })
            .unwrap(),
        )
        .unwrap();
}

fn register_new_buffer(lua: &Lua, globals: &mlua::Table) {
    globals
        .set(
            "new_buffer",
            lua.create_function(move |_lua, (width, height): (i32, i32)| {
                let text_screen = TextScreen::new((width, height));
                let screen: Box<dyn Screen> = Box::new(text_screen);
                mlua::Result::Ok(LuaLayer::new(Arc::new(Mutex::new(screen))))
            })
            .unwrap(),
        )
        .unwrap();
}

fn register_next_frame(lua: &Lua, globals: &mlua::Table, animator: &Arc<Mutex<Animator>>) {
    let a = animator.clone();
    globals
        .set(
            "next_frame",
            lua.create_function_mut(move |lua, buffer: Value| {
                if let Value::UserData(data) = &buffer {
                    lua.globals().set("cur_frame", a.lock().frames.len() + 2)?;
                    let monitor_type: usize = lua.globals().get("monitor_type")?;
                    a.lock().current_monitor_settings.monitor_type = MonitorType::from(monitor_type as i32);

                    a.lock().current_monitor_settings.gamma = lua.globals().get("monitor_gamma")?;
                    a.lock().current_monitor_settings.contrast = lua.globals().get("monitor_contrast")?;
                    a.lock().current_monitor_settings.saturation = lua.globals().get("monitor_saturation")?;
                    a.lock().current_monitor_settings.brightness = lua.globals().get("monitor_brightness")?;
                    a.lock().current_monitor_settings.blur = lua.globals().get("monitor_blur")?;
                    a.lock().current_monitor_settings.curvature = lua.globals().get("monitor_curvature")?;
                    a.lock().current_monitor_settings.scanlines = lua.globals().get("monitor_scanlines")?;

                    let lua_screen = data.borrow::<LuaLayer>()?;
                    a.lock().lua_next_frame(&lua_screen.screen)
                } else {
                    Err(mlua::Error::RuntimeError(format!("UserData parameter required, got: {:?}", buffer)))
                }
            })
            .unwrap(),
        )
        .unwrap();
}

fn register_delay_functions(lua: &Lua, globals: &mlua::Table, animator: &Arc<Mutex<Animator>>) {
    let luaanimator = animator.clone();
    globals
        .set(
            "get_delay",
            lua.create_function(move |_lua, ()| {
                let delay = luaanimator.lock().get_delay();
                mlua::Result::Ok(delay)
            })
            .unwrap(),
        )
        .unwrap();

    let luaanimator = animator.clone();
    globals
        .set(
            "set_delay",
            lua.create_function(move |_lua, delay: u32| {
                luaanimator.lock().set_delay(delay);
                mlua::Result::Ok(())
            })
            .unwrap(),
        )
        .unwrap();
}

fn register_log_function(lua: &Lua, globals: &mlua::Table, animator: &Arc<Mutex<Animator>>) {
    let luaanimator = animator.clone();
    globals
        .set(
            "log",
            lua.create_function(move |_lua, text: String| {
                if luaanimator.lock().log.len() < 1000 {
                    let frame = luaanimator.lock().frames.len();
                    luaanimator.lock().log.push(LogEntry { frame, text });
                }
                mlua::Result::Ok(())
            })
            .unwrap(),
        )
        .unwrap();
}

fn initialize_globals(globals: &mlua::Table, animator: &Arc<Mutex<Animator>>) {
    globals.set("cur_frame", 1).unwrap();

    let lock = animator.lock();
    let i: i32 = lock.current_monitor_settings.monitor_type as i32;
    globals.set("monitor_type", i).unwrap();
    globals.set("monitor_gamma", lock.current_monitor_settings.gamma).unwrap();
    globals.set("monitor_contrast", lock.current_monitor_settings.contrast).unwrap();
    globals.set("monitor_saturation", lock.current_monitor_settings.saturation).unwrap();
    globals.set("monitor_brightness", lock.current_monitor_settings.brightness).unwrap();
    globals.set("monitor_blur", lock.current_monitor_settings.blur).unwrap();
    globals.set("monitor_curvature", lock.current_monitor_settings.curvature).unwrap();
    globals.set("monitor_scanlines", lock.current_monitor_settings.scanlines).unwrap();
}
