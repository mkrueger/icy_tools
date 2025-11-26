//! Lua Buffer wrappers for scripting access to Screen types

use std::sync::Arc;

use icy_engine::{AttributedChar, Caret, Position, Screen, ScreenSink, TextBuffer, TextPane, attribute};
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

/// Wrapper around TextBuffer for Lua scripting access (used by Animator)
pub struct LuaBuffer {
    pub cur_layer: usize,
    pub caret: Caret,
    pub buffer: TextBuffer,
}

impl LuaBuffer {
    pub fn convert_from_unicode(&self, ch: String) -> mlua::Result<char> {
        let Some(ch) = ch.chars().next() else {
            return Err(mlua::Error::SyntaxError {
                message: "Empty string".to_string(),
                incomplete_input: false,
            });
        };

        let buffer_type = self.buffer.buffer_type;
        let ch = buffer_type.convert_from_unicode(ch);
        Ok(ch)
    }

    pub fn convert_to_unicode(&self, ch: AttributedChar) -> String {
        let buffer_type = self.buffer.buffer_type;
        let ch = buffer_type.convert_to_unicode(ch.ch);
        ch.to_string()
    }
}

impl UserData for LuaBuffer {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // Buffer dimensions
        fields.add_field_method_get("height", |_, this| Ok(this.buffer.get_height()));
        fields.add_field_method_set("height", |_, this, val| {
            this.buffer.set_height(val);
            Ok(())
        });
        fields.add_field_method_get("width", |_, this| Ok(this.buffer.get_width()));
        fields.add_field_method_set("width", |_, this, val| {
            this.buffer.set_width(val);
            Ok(())
        });

        // Font page
        fields.add_field_method_get("font_page", |_, this| Ok(this.caret.font_page()));
        fields.add_field_method_set("font_page", |_, this, val| {
            this.caret.set_font_page(val);
            Ok(())
        });

        // Current layer
        fields.add_field_method_get("layer", |_, this| Ok(this.cur_layer));
        fields.add_field_method_set("layer", |_, this, val| {
            if val < this.buffer.layers.len() {
                this.cur_layer = val;
                Ok(())
            } else {
                Err(layer_error(val, this.buffer.layers.len()))
            }
        });

        // Foreground color
        fields.add_field_method_get("fg", |_, this| Ok(this.caret.attribute.get_foreground()));
        fields.add_field_method_set("fg", |_, this, val| {
            this.caret.attribute.set_foreground(val);
            Ok(())
        });

        // Background color
        fields.add_field_method_get("bg", |_, this| Ok(this.caret.attribute.get_background()));
        fields.add_field_method_set("bg", |_, this, val| {
            this.caret.attribute.set_background(val);
            Ok(())
        });

        // Caret position
        fields.add_field_method_get("x", |_, this| Ok(this.caret.x));
        fields.add_field_method_set("x", |_, this, val| {
            this.caret.x = val;
            Ok(())
        });

        fields.add_field_method_get("y", |_, this| Ok(this.caret.y));
        fields.add_field_method_set("y", |_, this, val| {
            this.caret.y = val;
            Ok(())
        });

        // Layer offset
        fields.add_field_method_get("layer_x", |_, this| {
            if this.cur_layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[this.cur_layer].get_offset();
                Ok(offset.x)
            } else {
                Err(layer_error(this.cur_layer, this.buffer.layers.len()))
            }
        });

        fields.add_field_method_set("layer_x", |_, this, val| {
            if this.cur_layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[this.cur_layer].get_offset();
                this.buffer.layers[this.cur_layer].set_offset((val, offset.y));
                Ok(())
            } else {
                Err(layer_error(this.cur_layer, this.buffer.layers.len()))
            }
        });

        fields.add_field_method_get("layer_y", |_, this| {
            if this.cur_layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[this.cur_layer].get_offset();
                Ok(offset.y)
            } else {
                Err(layer_error(this.cur_layer, this.buffer.layers.len()))
            }
        });

        fields.add_field_method_set("layer_y", |_, this, val| {
            if this.cur_layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[this.cur_layer].get_offset();
                this.buffer.layers[this.cur_layer].set_offset((offset.x, val));
                Ok(())
            } else {
                Err(layer_error(this.cur_layer, this.buffer.layers.len()))
            }
        });

        fields.add_field_method_get("layer_count", |_, this| Ok(this.buffer.layers.len()));
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // Color methods
        methods.add_method_mut("fg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let color = this.buffer.palette.insert_color_rgb(r, g, b);
            this.caret.set_foreground(color);
            Ok(color)
        });

        methods.add_method_mut("bg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let color = this.buffer.palette.insert_color_rgb(r, g, b);
            this.caret.set_background(color);
            Ok(color)
        });

        methods.add_method_mut("set_palette_color", |_, this, (color, r, g, b): (u32, u8, u8, u8)| {
            this.buffer.palette.set_color_rgb(color, r, g, b);
            Ok(())
        });

        methods.add_method_mut("get_palette_color", |_, this, color: u32| {
            let (r, g, b) = this.buffer.palette.get_rgb(color);
            Ok([r, g, b])
        });

        // Character methods
        methods.add_method_mut("set_char", |_, this, (x, y, ch): (i32, i32, String)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }
            let mut attr = this.caret.attribute;
            attr.attr &= !attribute::INVISIBLE;
            let ch = AttributedChar::new(this.convert_from_unicode(ch)?, attr);
            this.buffer.layers[this.cur_layer].set_char(Position::new(x, y), ch);
            Ok(())
        });

        methods.add_method_mut("clear_char", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }
            this.buffer.layers[this.cur_layer].set_char(Position::new(x, y), AttributedChar::invisible());
            Ok(())
        });

        methods.add_method_mut("get_char", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }

            let ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            Ok(this.convert_to_unicode(ch))
        });

        methods.add_method_mut("pickup_char", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }

            let ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            let mut attr = ch.attribute;
            attr.attr &= !attribute::INVISIBLE;
            this.caret.attribute = attr;
            Ok(this.convert_to_unicode(ch))
        });

        // Foreground/background manipulation
        methods.add_method_mut("set_fg", |_, this, (x, y, col): (i32, i32, u32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }
            let mut ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            if !ch.is_visible() {
                ch.attribute.attr = 0;
            }
            ch.attribute.set_foreground(col);
            this.buffer.layers[this.cur_layer].set_char(Position::new(x, y), ch);
            Ok(())
        });

        methods.add_method_mut("get_fg", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }

            let ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            Ok(ch.attribute.get_foreground())
        });

        methods.add_method_mut("set_bg", |_, this, (x, y, col): (i32, i32, u32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }
            let mut ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            if !ch.is_visible() {
                ch.attribute.attr = 0;
            }
            ch.attribute.set_background(col);
            this.buffer.layers[this.cur_layer].set_char(Position::new(x, y), ch);
            Ok(())
        });

        methods.add_method_mut("get_bg", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }

            let ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            Ok(ch.attribute.get_background())
        });

        // Print and cursor methods
        methods.add_method_mut("print", |_, this, str: String| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(layer_error(this.cur_layer, this.buffer.layers.len()));
            }
            for c in str.chars() {
                let pos = this.caret.position();
                let mut attr = this.caret.attribute;
                attr.attr &= !attribute::INVISIBLE;

                let ch = AttributedChar::new(this.convert_from_unicode(c.to_string())?, attr);
                this.buffer.layers[this.cur_layer].set_char(pos, ch);
                this.caret.x += 1;
            }
            Ok(())
        });

        methods.add_method_mut("gotoxy", |_, this, (x, y): (i32, i32)| {
            this.caret.set_position(Position::new(x, y));
            Ok(())
        });

        // Layer manipulation methods
        methods.add_method_mut("set_layer_position", |_, this, (layer, x, y): (usize, i32, i32)| {
            if layer < this.buffer.layers.len() {
                this.buffer.layers[layer].set_offset((x, y));
                Ok(())
            } else {
                Err(layer_error(layer, this.buffer.layers.len()))
            }
        });

        methods.add_method_mut("set_layer_x_position", |_, this, (layer, x): (usize, i32)| {
            if layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[layer].get_offset();
                this.buffer.layers[layer].set_offset((x, offset.y));
                Ok(())
            } else {
                Err(layer_error(layer, this.buffer.layers.len()))
            }
        });

        methods.add_method_mut("set_layer_y_position", |_, this, (layer, y): (usize, i32)| {
            if layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[layer].get_offset();
                this.buffer.layers[layer].set_offset((offset.x, y));
                Ok(())
            } else {
                Err(layer_error(layer, this.buffer.layers.len()))
            }
        });

        methods.add_method_mut("get_layer_position", |_, this, layer: usize| {
            if layer < this.buffer.layers.len() {
                let pos = this.buffer.layers[layer].get_offset();
                Ok((pos.x, pos.y))
            } else {
                Err(layer_error(layer, this.buffer.layers.len()))
            }
        });

        methods.add_method_mut("set_layer_visible", |_, this, (layer, is_visible): (usize, bool)| {
            if layer < this.buffer.layers.len() {
                this.buffer.layers[layer].set_is_visible(is_visible);
                Ok(())
            } else {
                Err(layer_error(layer, this.buffer.layers.len()))
            }
        });

        methods.add_method_mut("get_layer_visible", |_, this, layer: usize| {
            if layer < this.buffer.layers.len() {
                Ok(this.buffer.layers[layer].get_is_visible())
            } else {
                Err(layer_error(layer, this.buffer.layers.len()))
            }
        });

        // Clear/reset methods
        methods.add_method_mut("clear", |_, this, ()| {
            this.caret = Caret::default();
            this.buffer.reset_terminal();
            Ok(())
        });
    }
}

/// Wrapper around Arc<Mutex<Box<dyn Screen>>> for Lua scripting access (used by icy_term)
pub struct LuaScreen {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
}

impl LuaScreen {
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

impl UserData for LuaScreen {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // Buffer dimensions
        fields.add_field_method_get("height", |_, this| Ok(this.screen.lock().get_height()));
        fields.add_field_method_set("height", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_height(val);
                Ok(())
            } else {
                Err(no_editable_error())
            }
        });

        fields.add_field_method_get("width", |_, this| Ok(this.screen.lock().get_width()));
        fields.add_field_method_set("width", |_, this, val| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                editable.set_size(icy_engine::Size::new(val, editable.get_height()));
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
        fields.add_field_method_get("fg", |_, this| Ok(this.screen.lock().caret().attribute.get_foreground()));
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
        fields.add_field_method_get("bg", |_, this| Ok(this.screen.lock().caret().attribute.get_background()));
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
                    Ok(layer.get_offset().x)
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
                    let offset = layer.get_offset();
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
                    Ok(layer.get_offset().y)
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
                    let offset = layer.get_offset();
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
            let (r, g, b) = screen.palette().get_rgb(color);
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
                    let ch = layer.get_char(Position::new(x, y));
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

        methods.add_method_mut("pickup_char", |_, this, (x, y): (i32, i32)| {
            let mut screen = this.screen.lock();
            if let Some(editable) = screen.as_editable() {
                let cur_layer = editable.get_current_layer();
                if let Some(layer) = editable.get_layer(cur_layer) {
                    let ch = layer.get_char(Position::new(x, y));
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
                    let mut ch = layer.get_char(Position::new(x, y));
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
                    let ch = layer.get_char(Position::new(x, y));
                    Ok(ch.attribute.get_foreground())
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
                    let mut ch = layer.get_char(Position::new(x, y));
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
                    let ch = layer.get_char(Position::new(x, y));
                    Ok(ch.attribute.get_background())
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
                    let offset = l.get_offset();
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
                    let offset = l.get_offset();
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
                    let pos = l.get_offset();
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
                    Ok(l.get_is_visible())
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
