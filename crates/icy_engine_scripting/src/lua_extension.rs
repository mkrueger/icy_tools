//! Lua extension trait for plugin support
//!
//! This module provides a trait that allows external crates to register
//! custom Lua functions and types into the scripting runtime.

use mlua::Lua;

/// Trait for extending the Lua scripting environment
///
/// Implement this trait to add custom functions, types, or globals
/// to the Lua runtime. This is useful for crates like `icy_term` that
/// need terminal-specific scripting capabilities.
///
/// # Example
///
/// ```ignore
/// use icy_engine_scripting::LuaExtension;
/// use mlua::Lua;
///
/// struct MyExtension;
///
/// impl LuaExtension for MyExtension {
///     fn register(&self, lua: &Lua) -> mlua::Result<()> {
///         let globals = lua.globals();
///         globals.set("my_function", lua.create_function(|_, ()| {
///             Ok("Hello from extension!")
///         })?)?;
///         Ok(())
///     }
/// }
/// ```
pub trait LuaExtension: Send + Sync {
    /// Register custom functions, types, and globals into the Lua runtime
    ///
    /// This method is called during Lua runtime initialization, allowing
    /// extensions to add their functionality before scripts are executed.
    fn register(&self, lua: &Lua) -> mlua::Result<()>;
}

/// A no-op extension for when no custom functionality is needed
pub struct NoExtension;

impl LuaExtension for NoExtension {
    fn register(&self, _lua: &Lua) -> mlua::Result<()> {
        Ok(())
    }
}
