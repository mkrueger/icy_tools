use std::{fs, path::Path, sync::Arc};

use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, Position, TextPane, UnicodeConverter, attribute};
use mlua::{Lua, UserData};
use regex::Regex;
use walkdir::WalkDir;

use crate::{Settings, model::font_imp::FontTool};

#[derive(Clone)]
pub struct Plugin {
    pub title: String,
    pub description: String,
    pub author: String,
    pub text: String,
    pub path: Vec<String>,
}

impl Plugin {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = fs::read_to_string(path)?;

        let re = Regex::new(r"--\s*Title:\s*(.*)")?;
        if let Some(cap) = re.captures(&text) {
            let title = cap.get(1).unwrap().as_str().to_string();

            let re = Regex::new(r"--\s*Author:\s*(.*)")?;
            let author = if let Some(cap) = re.captures(&text) {
                cap.get(1).unwrap().as_str().to_string()
            } else {
                String::new()
            };

            let re = Regex::new(r"--\s*Description:\s*(.*)")?;
            let description = if let Some(cap) = re.captures(&text) {
                cap.get(1).unwrap().as_str().to_string()
            } else {
                String::new()
            };

            let re = Regex::new(r"--\s*Path:\s*(.*)")?;
            let path = if let Some(cap) = re.captures(&text) {
                cap.get(1)
                    .unwrap()
                    .as_str()
                    .to_string()
                    .split('/')
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            } else {
                vec![]
            };

            return Ok(Self {
                title,
                author,
                description,
                text,
                path,
            });
        }
        Err(anyhow::anyhow!("No plugin file"))
    }

    pub(crate) fn run_plugin(&self, _window: &crate::MainWindow<'_>, editor: &crate::AnsiEditor) -> anyhow::Result<()> {
        let lua = Lua::new();
        let globals = lua.globals();

        globals
            .set(
                "log",
                lua.create_function(move |_lua, txt: String| {
                    log::info!("{txt}");
                    Ok(())
                })
                .map_err(|error| anyhow::anyhow!(error.to_string()))?,
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        globals
            .set(
                "buf",
                LuaBufferView {
                    buffer_view: editor.buffer_view.clone(),
                },
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        let sel = editor.buffer_view.lock().get_selection();

        let rect = if let Some(l) = editor.buffer_view.lock().get_edit_state().get_cur_layer() {
            l.get_rectangle()
        } else {
            return Err(anyhow::anyhow!("No layer selected"));
        };

        if let Some(sel) = sel {
            let mut selected_rect = sel.as_rectangle().intersect(&rect);
            selected_rect -= rect.start;

            globals
                .set("start_x", selected_rect.left())
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            globals
                .set("end_x", selected_rect.right() - 1)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            globals
                .set("start_y", selected_rect.top())
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            globals
                .set("end_y", selected_rect.bottom() - 1)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        } else {
            globals.set("start_x", 0).map_err(|error| anyhow::anyhow!(error.to_string()))?;
            globals.set("end_x", rect.get_width()).map_err(|error| anyhow::anyhow!(error.to_string()))?;
            globals.set("start_y", 0).map_err(|error| anyhow::anyhow!(error.to_string()))?;
            globals.set("end_y", rect.get_height()).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
        let _undo = editor
            .buffer_view
            .lock()
            .get_edit_state_mut()
            .begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-plugin", title = self.title.clone()));
        lua.load(&self.text).exec().map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(())
    }

    pub fn read_plugin_directory() -> Vec<Self> {
        let mut result = Vec::new();
        let Ok(root) = Settings::get_plugin_directory() else {
            log::error!("Can't read plugin directory.");
            return result;
        };
        let walker = WalkDir::new(root).into_iter();
        for entry in walker.filter_entry(|e| !FontTool::is_hidden(e)) {
            match entry {
                Ok(entry) => {
                    if entry.file_type().is_dir() {
                        continue;
                    }
                    match Plugin::load(entry.path()) {
                        Ok(plugin) => {
                            result.push(plugin);
                        }
                        Err(err) => log::error!("Error loading plugin: {err}"),
                    }
                }
                Err(err) => log::error!("Error loading plugin: {err}"),
            }
        }
        result
    }
}

struct LuaBufferView {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<icy_engine_gui::BufferView>>,
}

impl LuaBufferView {
    fn convert_from_unicode(&self, ch: String) -> mlua::Result<char> {
        let Some(ch) = ch.chars().next() else {
            return Err(mlua::Error::SyntaxError {
                message: "Empty string".to_string(),
                incomplete_input: false,
            });
        };

        let buffer_type = self.buffer_view.lock().get_buffer().buffer_type;
        let ch = match buffer_type {
            icy_engine::BufferType::Unicode => ch,
            icy_engine::BufferType::CP437 => {
                icy_engine::ascii::CP437Converter::default().convert_from_unicode(ch, self.buffer_view.lock().get_caret().get_font_page())
            }
            icy_engine::BufferType::Petscii => {
                icy_engine::petscii::CharConverter::default().convert_from_unicode(ch, self.buffer_view.lock().get_caret().get_font_page())
            }
            icy_engine::BufferType::Atascii => {
                icy_engine::atascii::CharConverter::default().convert_from_unicode(ch, self.buffer_view.lock().get_caret().get_font_page())
            }
            icy_engine::BufferType::Viewdata => {
                icy_engine::viewdata::CharConverter::default().convert_from_unicode(ch, self.buffer_view.lock().get_caret().get_font_page())
            }
        };
        Ok(ch)
    }

    fn convert_to_unicode(&self, ch: AttributedChar) -> String {
        let buffer_type = self.buffer_view.lock().get_buffer().buffer_type;
        let ch = match buffer_type {
            icy_engine::BufferType::Unicode => ch.ch,
            icy_engine::BufferType::CP437 => icy_engine::ascii::CP437Converter::default().convert_to_unicode(ch),
            icy_engine::BufferType::Petscii => icy_engine::petscii::CharConverter::default().convert_to_unicode(ch),
            icy_engine::BufferType::Atascii => icy_engine::atascii::CharConverter::default().convert_to_unicode(ch),
            icy_engine::BufferType::Viewdata => icy_engine::viewdata::CharConverter::default().convert_to_unicode(ch),
        };
        ch.to_string()
    }
}

impl UserData for LuaBufferView {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("height", |_, this| Ok(this.buffer_view.lock().get_buffer_mut().get_height()));
        fields.add_field_method_set("height", |_, this, val| {
            this.buffer_view.lock().get_buffer_mut().set_height(val);
            Ok(())
        });
        fields.add_field_method_get("width", |_, this| Ok(this.buffer_view.lock().get_buffer_mut().get_width()));
        fields.add_field_method_set("width", |_, this, val| {
            this.buffer_view.lock().get_buffer_mut().set_width(val);
            Ok(())
        });

        fields.add_field_method_get("font_page", |_, this| Ok(this.buffer_view.lock().get_caret_mut().get_font_page()));
        fields.add_field_method_set("font_page", |_, this, val| {
            this.buffer_view.lock().get_caret_mut().set_font_page(val);
            Ok(())
        });

        fields.add_field_method_get("layer", |_, this| Ok(this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap()));
        fields.add_field_method_set("layer", |_, this, val| {
            if val < this.buffer_view.lock().get_buffer_mut().layers.len() {
                this.buffer_view.lock().get_edit_state_mut().set_current_layer(val);
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", val, this.buffer_view.lock().get_buffer_mut().layers.len()),
                    incomplete_input: false,
                })
            }
        });

        fields.add_field_method_get("fg", |_, this| Ok(this.buffer_view.lock().get_caret_mut().get_attribute().get_foreground()));
        fields.add_field_method_set("fg", |_, this, val| {
            let mut attr = this.buffer_view.lock().get_caret_mut().get_attribute();
            attr.set_foreground(val);
            this.buffer_view.lock().get_caret_mut().set_attr(attr);
            Ok(())
        });

        fields.add_field_method_get("bg", |_, this| Ok(this.buffer_view.lock().get_caret_mut().get_attribute().get_background()));
        fields.add_field_method_set("bg", |_, this, val| {
            let mut attr = this.buffer_view.lock().get_caret_mut().get_attribute();
            attr.set_background(val);
            this.buffer_view.lock().get_caret_mut().set_attr(attr);
            Ok(())
        });

        fields.add_field_method_get("x", |_, this| Ok(this.buffer_view.lock().get_caret_mut().get_position().x));
        fields.add_field_method_set("x", |_, this, val| {
            this.buffer_view.lock().get_caret_mut().set_x_position(val);
            Ok(())
        });

        fields.add_field_method_get("y", |_, this| Ok(this.buffer_view.lock().get_caret_mut().get_position().y));
        fields.add_field_method_set("y", |_, this, val| {
            this.buffer_view.lock().get_caret_mut().set_y_position(val);
            Ok(())
        });

        fields.add_field_method_get("layer_count", |_, this| Ok(this.buffer_view.lock().get_buffer_mut().layers.len()));
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("fg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let color = this.buffer_view.lock().get_buffer_mut().palette.insert_color_rgb(r, g, b);
            this.buffer_view.lock().get_caret_mut().set_foreground(color);
            Ok(color)
        });

        methods.add_method_mut("bg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            let color = this.buffer_view.lock().get_buffer_mut().palette.insert_color_rgb(r, g, b);
            this.buffer_view.lock().get_caret_mut().set_background(color);
            Ok(color)
        });

        methods.add_method_mut("set_char", |_, this, (x, y, ch): (i32, i32, String)| {
            let cur_layer = this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap();
            let layer_len = this.buffer_view.lock().get_buffer_mut().layers.len();
            if cur_layer >= layer_len {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                    incomplete_input: false,
                });
            }
            let mut attr = this.buffer_view.lock().get_caret_mut().get_attribute();
            attr.attr &= !attribute::INVISIBLE;
            let ch = AttributedChar::new(this.convert_from_unicode(ch)?, attr);

            if let Err(err) = this.buffer_view.lock().get_edit_state_mut().set_char((x, y), ch) {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Error setting char: {}", err),
                    incomplete_input: false,
                });
            };
            Ok(())
        });

        methods.add_method_mut("get_char", |_, this, (x, y): (i32, i32)| {
            let cur_layer = this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap();
            let layer_len = this.buffer_view.lock().get_buffer_mut().layers.len();
            if cur_layer >= layer_len {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                    incomplete_input: false,
                });
            }

            let ch = this.buffer_view.lock().get_buffer_mut().layers[cur_layer].get_char((x, y));
            Ok(this.convert_to_unicode(ch))
        });

        methods.add_method_mut("pickup_char", |_, this, (x, y): (i32, i32)| {
            let cur_layer = this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap();
            let layer_len = this.buffer_view.lock().get_buffer_mut().layers.len();
            if cur_layer >= layer_len {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                    incomplete_input: false,
                });
            }

            let ch = this.buffer_view.lock().get_buffer_mut().layers[cur_layer].get_char((x, y));
            let mut attr = ch.attribute;
            attr.attr &= !attribute::INVISIBLE;
            this.buffer_view.lock().get_caret_mut().set_attr(attr);

            Ok(this.convert_to_unicode(ch))
        });

        methods.add_method_mut("set_fg", |_, this, (x, y, col): (i32, i32, u32)| {
            let cur_layer = this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap();
            let layer_len = this.buffer_view.lock().get_buffer_mut().layers.len();
            if cur_layer >= layer_len {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                    incomplete_input: false,
                });
            }
            let mut ch = this.buffer_view.lock().get_buffer_mut().layers[cur_layer].get_char((x, y));
            ch.attribute.set_foreground(col);
            if let Err(err) = this.buffer_view.lock().get_edit_state_mut().set_char((x, y), ch) {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Error setting char: {}", err),
                    incomplete_input: false,
                });
            }
            Ok(())
        });

        methods.add_method_mut("get_fg", |_, this, (x, y): (i32, i32)| {
            let cur_layer = this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap();
            let layer_len = this.buffer_view.lock().get_buffer_mut().layers.len();
            if cur_layer >= layer_len {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                    incomplete_input: false,
                });
            }

            let ch = this.buffer_view.lock().get_buffer_mut().layers[cur_layer].get_char((x, y));
            Ok(ch.attribute.get_foreground())
        });

        methods.add_method_mut("set_bg", |_, this, (x, y, col): (i32, i32, u32)| {
            let cur_layer = this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap();
            let layer_len = this.buffer_view.lock().get_buffer_mut().layers.len();
            if cur_layer >= layer_len {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                    incomplete_input: false,
                });
            }
            let mut ch = this.buffer_view.lock().get_buffer_mut().layers[cur_layer].get_char((x, y));
            ch.attribute.set_background(col);
            if let Err(err) = this.buffer_view.lock().get_edit_state_mut().set_char((x, y), ch) {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Error setting char: {}", err),
                    incomplete_input: false,
                });
            }
            Ok(())
        });

        methods.add_method_mut("get_bg", |_, this, (x, y): (i32, i32)| {
            let cur_layer = this.buffer_view.lock().get_edit_state_mut().get_current_layer().unwrap();
            let layer_len = this.buffer_view.lock().get_buffer_mut().layers.len();
            if cur_layer >= layer_len {
                return Err(mlua::Error::SyntaxError {
                    message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                    incomplete_input: false,
                });
            }
            let ch = this.buffer_view.lock().get_buffer_mut().layers[cur_layer].get_char((x, y));
            Ok(ch.attribute.get_background())
        });

        methods.add_method_mut("print", |_, this, str: String| {
            for c in str.chars() {
                let mut pos = this.buffer_view.lock().get_caret_mut().get_position();
                let mut attribute = this.buffer_view.lock().get_caret_mut().get_attribute();
                attribute.attr &= !attribute::INVISIBLE;
                let ch = AttributedChar::new(this.convert_from_unicode(c.to_string())?, attribute);
                let _ = this.buffer_view.lock().get_edit_state_mut().set_char(pos, ch);
                pos.x += 1;
                this.buffer_view.lock().get_caret_mut().set_position(pos);
            }
            Ok(())
        });

        methods.add_method_mut("gotoxy", |_, this, (x, y): (i32, i32)| {
            this.buffer_view.lock().get_caret_mut().set_position(Position::new(x, y));
            Ok(())
        });

        methods.add_method_mut("set_layer_position", |_, this, (layer, x, y): (usize, i32, i32)| {
            if layer < this.buffer_view.lock().get_buffer_mut().layers.len() {
                let _ = this.buffer_view.lock().get_edit_state_mut().move_layer(Position::new(x, y));
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer_view.lock().get_buffer_mut().layers.len()),
                    incomplete_input: false,
                })
            }
        });
        methods.add_method_mut("get_layer_position", |_, this, layer: usize| {
            if layer < this.buffer_view.lock().get_buffer_mut().layers.len() {
                let pos = this.buffer_view.lock().get_buffer_mut().layers[layer].get_offset();
                Ok((pos.x, pos.y))
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer_view.lock().get_buffer_mut().layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("set_layer_visible", |_, this, (layer, is_visible): (i32, bool)| {
            let layer = layer as usize;
            if layer < this.buffer_view.lock().get_buffer_mut().layers.len() {
                // todo
                this.buffer_view.lock().get_buffer_mut().layers[layer].set_is_visible(is_visible);
                Ok(())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer_view.lock().get_buffer_mut().layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("get_layer_visible", |_, this, layer: usize| {
            if layer < this.buffer_view.lock().get_buffer_mut().layers.len() {
                Ok(this.buffer_view.lock().get_buffer_mut().layers[layer].get_is_visible())
            } else {
                Err(mlua::Error::SyntaxError {
                    message: format!("Layer {} out of range (0..<{})", layer, this.buffer_view.lock().get_buffer_mut().layers.len()),
                    incomplete_input: false,
                })
            }
        });

        methods.add_method_mut("clear", |_, this, ()| {
            this.buffer_view.lock().get_buffer_mut().reset_terminal();
            Ok(())
        });
    }
}
