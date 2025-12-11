//! Lua Layer and Screen wrappers for scripting access to Screen types
//!
//! - `LuaLayer`: Base API for layer manipulation (characters, colors, layer position)
//!   Used by icy_draw for animations.
//! - `LuaScreen`: Extends `LuaLayer` with caret movement and terminal functions.
//!   Used by icy_term for terminal scripting.

use std::sync::Arc;

use icy_engine::{AttributedChar, Position, Screen, ScreenSink, TextPane, attribute};
use icy_parser_core::{CommandParser, PcBoardParser};
use mlua::UserData;
use parking_lot::Mutex;

/// Helper functions for layer error handling
fn layer_error(layer: usize, count: usize) -> mlua::Error {
    mlua::Error::SyntaxError {
        message: format!("Layer {} out of range (0..<{})", layer, count),
        incomplete_input: false,
    }
}

fn no_editable_error() -> mlua::Error {
    mlua::Error::RuntimeError("Screen is not editable".to_string())
}

/// Wrapper around Arc<Mutex<Box<dyn Screen>>> for Lua scripting access to layers.
/// Provides character, color, and layer manipulation - used by icy_draw animations.
pub struct LuaLayer {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
}

impl LuaLayer {
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        Self { screen }
    }

    pub fn convert_from_unicode(&self, ch: String) -> mlua::Result<char> {
        let Some(ch) = ch.chars().next() else {
            return Err(mlua::Error::SyntaxError {
                message: "Empty string".to_string(),
                incomplete_input: false,
            });
        };

        let screen = self.screen.lock();
        let buffer_type = screen.buffer_type();
        let ch = buffer_type.convert_from_unicode(ch);
        Ok(ch)
    }

    pub fn convert_to_unicode(&self, ch: AttributedChar) -> String {
        let screen = self.screen.lock();
        let buffer_type = screen.buffer_type();
        let ch = buffer_type.convert_to_unicode(ch.ch);
        ch.to_string()
    }
}

impl UserData for LuaLayer {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // Buffer dimensions
        fields.add_field_method_get("height", |_, this| Ok(this.screen.lock().height()));
        fields.add_field_method_set("height", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_height(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("width", |_, this| Ok(this.screen.lock().width()));
        fields.add_field_method_set("width", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_size(icy_engine::Size::new(val, editable.height()));
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Font page
        fields.add_field_method_get("font_page", |_, this| Ok(this.screen.lock().caret().font_page()));
        fields.add_field_method_set("font_page", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().set_font_page(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Current layer
        fields.add_field_method_get("layer", |_, this| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                Ok(editable.get_current_layer())
            } else {
                Ok(0)
            }
        });
        fields.add_field_method_set("layer", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_current_layer(val).map_err(|e| mlua::Error::SyntaxError {
                    message: e.to_string(),
                    incomplete_input: false,
                })
            } else {
                Err(no_editable_error())
            }
        });

        // Foreground color
        fields.add_field_method_get("fg", |_, this| Ok(this.screen.lock().caret().attribute.foreground()));
        fields.add_field_method_set("fg", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().attribute.set_foreground(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Background color
        fields.add_field_method_get("bg", |_, this| Ok(this.screen.lock().caret().attribute.background()));
        fields.add_field_method_set("bg", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().attribute.set_background(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Caret position
        fields.add_field_method_get("x", |_, this| Ok(this.screen.lock().caret().x));
        fields.add_field_method_set("x", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().x = val;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("y", |_, this| Ok(this.screen.lock().caret().y));
        fields.add_field_method_set("y", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().y = val;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Layer offset
        fields.add_field_method_get("layer_x", |_, this| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    Ok(layer.offset().x)
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_set("layer_x", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let offset = layer.offset();
                    layer.set_offset((val, offset.y));
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("layer_y", |_, this| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    Ok(layer.offset().y)
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_set("layer_y", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let offset = layer.offset();
                    layer.set_offset((offset.x, val));
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("layer_count", |_, this| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                Ok(editable.layer_count())
            } else {
                Ok(1)
            }
        });
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // Color methods
        methods.add_method_mut("fg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let color = editable.palette_mut().insert_color_rgb(r, g, b);
                editable.caret_mut().set_foreground(color);
                Ok(color)
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("bg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let color = editable.palette_mut().insert_color_rgb(r, g, b);
                editable.caret_mut().set_background(color);
                Ok(color)
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_palette_color", |_, this, (color, r, g, b): (u32, u8, u8, u8)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.palette_mut().set_color_rgb(color, r, g, b);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_palette_color", |_, this, color: u32| {
            let screen = this.screen.lock();
            let (r, g, b) = screen.palette().rgb(color);
            Ok([r, g, b])
        });

        // Character methods
        methods.add_method_mut("set_char", |_, this, (x, y, ch): (i32, i32, String)| {
            let ch = this.convert_from_unicode(ch)?;
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                let layer_count = editable.layer_count();
                let mut attr = editable.caret().attribute;
                attr.attr &= !attribute::INVISIBLE;
                let ch = AttributedChar::new(ch, attr);
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    layer.set_char(Position::new(x, y), ch);
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, layer_count))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("clear_char", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    layer.set_char(Position::new(x, y), AttributedChar::invisible());
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_char", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    let buffer_type = editable.buffer_type();
                    let ch = buffer_type.convert_to_unicode(ch.ch);
                    Ok(ch.to_string())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("grab_char", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    let mut attr = ch.attribute;
                    attr.attr &= !attribute::INVISIBLE;
                    editable.caret_mut().attribute = attr;
                    let buffer_type = editable.buffer_type();
                    let ch = buffer_type.convert_to_unicode(ch.ch);
                    Ok(ch.to_string())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        // Foreground/background manipulation
        methods.add_method_mut("set_fg", |_, this, (x, y, col): (i32, i32, u32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let mut ch = layer.char_at(Position::new(x, y));
                    if !ch.is_visible() {
                        ch.attribute.attr = 0;
                    }
                    ch.attribute.set_foreground(col);
                    layer.set_char(Position::new(x, y), ch);
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_fg", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    Ok(ch.attribute.foreground())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_bg", |_, this, (x, y, col): (i32, i32, u32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let mut ch = layer.char_at(Position::new(x, y));
                    if !ch.is_visible() {
                        ch.attribute.attr = 0;
                    }
                    ch.attribute.set_background(col);
                    layer.set_char(Position::new(x, y), ch);
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_bg", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    Ok(ch.attribute.background())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        // Print and cursor methods - uses PCBoard parser for color codes like @X0F
        methods.add_method_mut("print", |_, this, str: String| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let mut parser = PcBoardParser::new();
                let mut sink = ScreenSink::new(editable);
                parser.parse(str.as_bytes(), &mut sink);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("println", |_, this, str: String| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let mut parser = PcBoardParser::new();
                let mut sink = ScreenSink::new(editable);
                parser.parse(str.as_bytes(), &mut sink);
                // Move to next line
                let pos = editable.caret().position();
                editable.caret_mut().set_position(Position::new(0, pos.y + 1));
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("gotoxy", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().set_position(Position::new(x, y));
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Layer manipulation methods
        methods.add_method_mut("set_layer_position", |_, this, (layer, x, y): (usize, i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    l.set_offset((x, y));
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_layer_x_position", |_, this, (layer, x): (usize, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    let offset = l.offset();
                    l.set_offset((x, offset.y));
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_layer_y_position", |_, this, (layer, y): (usize, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    let offset = l.offset();
                    l.set_offset((offset.x, y));
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_layer_position", |_, this, layer: usize| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer(layer) {
                    let pos = l.offset();
                    Ok((pos.x, pos.y))
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_layer_visible", |_, this, (layer, is_visible): (usize, bool)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    l.set_is_visible(is_visible);
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_layer_visible", |_, this, layer: usize| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer(layer) {
                    Ok(l.is_visible())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        // Clear/reset methods
        methods.add_method_mut("clear", |_, this, ()| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.clear_screen();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });
    }
}

/// Wrapper around LuaLayer with additional caret movement and terminal functions.
/// Used by icy_term for terminal scripting. Provides all LuaLayer functionality
/// plus caret positioning, movement, and state management.
pub struct LuaScreen {
    pub layer: LuaLayer,
}

impl LuaScreen {
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        Self { layer: LuaLayer::new(screen) }
    }

    /// Get a reference to the underlying screen
    pub fn screen(&self) -> &Arc<Mutex<Box<dyn Screen>>> {
        &self.layer.screen
    }
}

impl UserData for LuaScreen {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // Delegate all LuaLayer fields
        // Buffer dimensions
        fields.add_field_method_get("height", |_, this| Ok(this.layer.screen.lock().height()));
        fields.add_field_method_set("height", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_height(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("width", |_, this| Ok(this.layer.screen.lock().width()));
        fields.add_field_method_set("width", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_size(icy_engine::Size::new(val, editable.height()));
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Font page
        fields.add_field_method_get("font_page", |_, this| Ok(this.layer.screen.lock().caret().font_page()));
        fields.add_field_method_set("font_page", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().set_font_page(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Current layer
        fields.add_field_method_get("layer", |_, this| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                Ok(editable.get_current_layer())
            } else {
                Ok(0)
            }
        });
        fields.add_field_method_set("layer", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_current_layer(val).map_err(|e| mlua::Error::SyntaxError {
                    message: e.to_string(),
                    incomplete_input: false,
                })
            } else {
                Err(no_editable_error())
            }
        });

        // Foreground color
        fields.add_field_method_get("fg", |_, this| Ok(this.layer.screen.lock().caret().attribute.foreground()));
        fields.add_field_method_set("fg", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().attribute.set_foreground(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Background color
        fields.add_field_method_get("bg", |_, this: &LuaScreen| Ok(this.layer.screen.lock().caret().attribute.background()));
        fields.add_field_method_set("bg", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().attribute.set_background(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Caret position
        fields.add_field_method_get("x", |_, this| Ok(this.layer.screen.lock().caret().x));
        fields.add_field_method_set("x", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().x = val;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("y", |_, this| Ok(this.layer.screen.lock().caret().y));
        fields.add_field_method_set("y", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().y = val;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Layer offset
        fields.add_field_method_get("layer_x", |_, this| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    Ok(layer.offset().x)
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_set("layer_x", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let offset = layer.offset();
                    layer.set_offset((val, offset.y));
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("layer_y", |_, this| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    Ok(layer.offset().y)
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_set("layer_y", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let offset = layer.offset();
                    layer.set_offset((offset.x, val));
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("layer_count", |_, this| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                Ok(editable.layer_count())
            } else {
                Ok(1)
            }
        });

        // Caret properties (LuaScreen-specific)
        fields.add_field_method_get("insert_mode", |_, this| Ok(this.layer.screen.lock().caret().insert_mode));
        fields.add_field_method_set("insert_mode", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().insert_mode = val;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("caret_visible", |_, this| Ok(this.layer.screen.lock().caret().visible));
        fields.add_field_method_set("caret_visible", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().visible = val;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("caret_blinking", |_, this| Ok(this.layer.screen.lock().caret().blinking));
        fields.add_field_method_set("caret_blinking", |_, this, val| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().blinking = val;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // ============ Delegated LuaLayer methods ============

        // Color methods
        methods.add_method_mut("fg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let color = editable.palette_mut().insert_color_rgb(r, g, b);
                editable.caret_mut().set_foreground(color);
                Ok(color)
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("bg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let color = editable.palette_mut().insert_color_rgb(r, g, b);
                editable.caret_mut().set_background(color);
                Ok(color)
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_palette_color", |_, this, (color, r, g, b): (u32, u8, u8, u8)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.palette_mut().set_color_rgb(color, r, g, b);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_palette_color", |_, this, color: u32| {
            let screen = this.layer.screen.lock();
            let (r, g, b) = screen.palette().rgb(color);
            Ok([r, g, b])
        });

        // Character methods
        methods.add_method_mut("set_char", |_, this, (x, y, ch): (i32, i32, String)| {
            let ch = this.layer.convert_from_unicode(ch)?;
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                let layer_count = editable.layer_count();
                let mut attr = editable.caret().attribute;
                attr.attr &= !attribute::INVISIBLE;
                let ch = AttributedChar::new(ch, attr);
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    layer.set_char(Position::new(x, y), ch);
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, layer_count))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("clear_char", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    layer.set_char(Position::new(x, y), AttributedChar::invisible());
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_char", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    let buffer_type = editable.buffer_type();
                    let ch = buffer_type.convert_to_unicode(ch.ch);
                    Ok(ch.to_string())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("grab_char", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    let mut attr = ch.attribute;
                    attr.attr &= !attribute::INVISIBLE;
                    editable.caret_mut().attribute = attr;
                    let buffer_type = editable.buffer_type();
                    let ch = buffer_type.convert_to_unicode(ch.ch);
                    Ok(ch.to_string())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        // Foreground/background manipulation
        methods.add_method_mut("set_fg", |_, this, (x, y, col): (i32, i32, u32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let mut ch = layer.char_at(Position::new(x, y));
                    if !ch.is_visible() {
                        ch.attribute.attr = 0;
                    }
                    ch.attribute.set_foreground(col);
                    layer.set_char(Position::new(x, y), ch);
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_fg", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    Ok(ch.attribute.foreground())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_bg", |_, this, (x, y, col): (i32, i32, u32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer_mut(cur_layer) {
                    let mut ch = layer.char_at(Position::new(x, y));
                    if !ch.is_visible() {
                        ch.attribute.attr = 0;
                    }
                    ch.attribute.set_background(col);
                    layer.set_char(Position::new(x, y), ch);
                    Ok(())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_bg", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.char_at(Position::new(x, y));
                    Ok(ch.attribute.background())
                } else {
                    Err(layer_error(cur_layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        // Print and cursor methods
        methods.add_method_mut("print", |_, this, str: String| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let mut parser = PcBoardParser::new();
                let mut sink = ScreenSink::new(editable);
                parser.parse(str.as_bytes(), &mut sink);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("println", |_, this, str: String| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let mut parser = PcBoardParser::new();
                let mut sink = ScreenSink::new(editable);
                parser.parse(str.as_bytes(), &mut sink);
                // Move to next line
                let pos = editable.caret().position();
                editable.caret_mut().set_position(Position::new(0, pos.y + 1));
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("gotoxy", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.caret_mut().set_position(Position::new(x, y));
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Layer manipulation methods
        methods.add_method_mut("set_layer_position", |_, this, (layer, x, y): (usize, i32, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    l.set_offset((x, y));
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_layer_x_position", |_, this, (layer, x): (usize, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    let offset = l.offset();
                    l.set_offset((x, offset.y));
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_layer_y_position", |_, this, (layer, y): (usize, i32)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    let offset = l.offset();
                    l.set_offset((offset.x, y));
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_layer_position", |_, this, layer: usize| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer(layer) {
                    let pos = l.offset();
                    Ok((pos.x, pos.y))
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("set_layer_visible", |_, this, (layer, is_visible): (usize, bool)| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer_mut(layer) {
                    l.set_is_visible(is_visible);
                    Ok(())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method("get_layer_visible", |_, this, layer: usize| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                if let Some(l) = editable.get_layer(layer) {
                    Ok(l.is_visible())
                } else {
                    Err(layer_error(layer, editable.layer_count()))
                }
            } else {
                Err(no_editable_error())
            }
        });

        // Clear/reset methods
        methods.add_method_mut("clear", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.clear_screen();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // ============ LuaScreen-specific caret methods ============

        // Caret movement
        methods.add_method_mut("caret_left", |_, this, n: Option<i32>| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.left(n.unwrap_or(1), false, false);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_right", |_, this, n: Option<i32>| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.right(n.unwrap_or(1), false, false);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_up", |_, this, n: Option<i32>| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.up(n.unwrap_or(1), false, false);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_down", |_, this, n: Option<i32>| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.down(n.unwrap_or(1), false, false);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Caret control
        methods.add_method_mut("caret_home", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.home();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_eol", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.eol();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_cr", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.cr();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_lf", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.lf();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_next_line", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.next_line(true);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_bs", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.bs();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_del", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.del();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_ins", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.ins();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("caret_tab", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.tab_forward();
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        // Save/restore caret state
        methods.add_method_mut("save_caret", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let caret = editable.caret().clone();
                let state = editable.saved_cursor_state();
                state.caret = caret;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        methods.add_method_mut("restore_caret", |_, this, ()| {
            let mut screen = this.layer.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let saved_caret = editable.saved_cursor_state().caret.clone();
                *editable.caret_mut() = saved_caret;
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });
    }
}
