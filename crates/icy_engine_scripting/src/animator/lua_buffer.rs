//! Lua Buffer wrapper for scripting access to TextBuffer

use icy_engine::{AttributedChar, Caret, Position, TextBuffer, TextPane, attribute};
use mlua::UserData;

/// Wrapper around TextBuffer for Lua scripting access
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
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", val, this.buffer.layers.len()),
                    incomplete_input: false,
                })
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
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        fields.add_field_method_set("layer_x", |_, this, val| {
            if this.cur_layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[this.cur_layer].get_offset();
                this.buffer.layers[this.cur_layer].set_offset((val, offset.y));
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        fields.add_field_method_get("layer_y", |_, this| {
            if this.cur_layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[this.cur_layer].get_offset();
                Ok(offset.y)
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        fields.add_field_method_set("layer_y", |_, this, val| {
            if this.cur_layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[this.cur_layer].get_offset();
                this.buffer.layers[this.cur_layer].set_offset((offset.x, val));
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
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
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
            }
            let mut attr = this.caret.attribute;
            attr.attr &= !attribute::INVISIBLE;
            let ch = AttributedChar::new(this.convert_from_unicode(ch)?, attr);
            this.buffer.layers[this.cur_layer].set_char(Position::new(x, y), ch);
            Ok(())
        });

        methods.add_method_mut("clear_char", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
            }
            this.buffer.layers[this.cur_layer].set_char(Position::new(x, y), AttributedChar::invisible());
            Ok(())
        });

        methods.add_method_mut("get_char", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
            }

            let ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            Ok(this.convert_to_unicode(ch))
        });

        methods.add_method_mut("pickup_char", |_, this, (x, y): (i32, i32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
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
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
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
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
            }

            let ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            Ok(ch.attribute.get_foreground())
        });

        methods.add_method_mut("set_bg", |_, this, (x, y, col): (i32, i32, u32)| {
            if this.cur_layer >= this.buffer.layers.len() {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
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
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", this.cur_layer, this.buffer.layers.len()),
                    incomplete_input: false,
                });
            }

            let ch = this.buffer.layers[this.cur_layer].get_char(Position::new(x, y));
            Ok(ch.attribute.get_background())
        });

        // Print and cursor methods
        methods.add_method_mut("print", |_, this, str: String| {
            for c in str.chars() {
                let mut pos = this.caret.position();
                let mut attr = this.caret.attribute;
                attr.attr &= !attribute::INVISIBLE;

                let ch = AttributedChar::new(this.convert_from_unicode(c.to_string())?, attr);

                this.buffer.layers[this.cur_layer].set_char(pos, ch);
                pos.x += 1;
                this.caret.set_position(pos);
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
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("set_layer_x_position", |_, this, (layer, x): (usize, i32)| {
            if layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[layer].get_offset();
                this.buffer.layers[layer].set_offset((x, offset.y));
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("set_layer_y_position", |_, this, (layer, y): (usize, i32)| {
            if layer < this.buffer.layers.len() {
                let offset = this.buffer.layers[layer].get_offset();
                this.buffer.layers[layer].set_offset((offset.x, y));
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("get_layer_position", |_, this, layer: usize| {
            if layer < this.buffer.layers.len() {
                let pos = this.buffer.layers[layer].get_offset();
                Ok((pos.x, pos.y))
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("set_layer_visible", |_, this, (layer, is_visible): (i32, bool)| {
            let layer = layer as usize;
            if layer < this.buffer.layers.len() {
                this.buffer.layers[layer].set_is_visible(is_visible);
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("get_layer_visible", |_, this, layer: usize| {
            if layer < this.buffer.layers.len() {
                Ok(this.buffer.layers[layer].get_is_visible())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer.layers.len()),
                    incomplete_input: false,
                })
            }
        });

        // Clear buffer
        methods.add_method_mut("clear", |_, this, ()| {
            this.caret = Caret::default();
            this.buffer = TextBuffer::new(this.buffer.get_size());
            Ok(())
        });
    }
}
