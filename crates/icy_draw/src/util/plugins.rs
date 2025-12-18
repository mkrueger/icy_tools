//! Plugin system for icy_draw
//!
//! Plugins are Lua scripts that can manipulate the buffer.
//! They are loaded from the plugin directory and shown in the Extensions menu.

use std::{collections::HashMap, fs, path::Path};

use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, Position, TextPane, attribute};
use icy_engine_edit::EditState;
use mlua::{Lua, UserData};
use parking_lot::Mutex;
use regex::Regex;
use std::sync::Arc;
use walkdir::WalkDir;

use crate::LANGUAGE_LOADER;
use crate::Settings;

/// A Lua plugin that can be run on the buffer
#[derive(Clone)]
pub struct Plugin {
    /// Display title for the menu
    pub title: String,
    /// Description shown on hover
    pub description: String,
    /// Plugin author
    pub author: String,
    /// The Lua script code
    pub text: String,
    /// Menu path (for submenu organization)
    pub path: Vec<String>,
}

impl Plugin {
    /// Load a plugin from a Lua file
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

    /// Run the plugin on the given edit state
    pub fn run_plugin(&self, screen: &Arc<Mutex<Box<dyn icy_engine::Screen>>>) -> anyhow::Result<()> {
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
            .set("buf", LuaBufferView { screen: screen.clone() })
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        // Get selection bounds or layer bounds
        let (start_x, end_x, start_y, end_y) = {
            let mut screen_guard = screen.lock();
            let edit_state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .ok_or_else(|| anyhow::anyhow!("Screen is not EditState"))?;

            let sel = edit_state.selection();
            let rect = if let Some(l) = edit_state.get_cur_layer() {
                l.rectangle()
            } else {
                return Err(anyhow::anyhow!("No layer selected"));
            };

            if let Some(sel) = sel {
                let mut selected_rect = sel.as_rectangle().intersect(&rect);
                selected_rect -= rect.start;
                (selected_rect.left(), selected_rect.right() - 1, selected_rect.top(), selected_rect.bottom() - 1)
            } else {
                (0, rect.width(), 0, rect.height())
            }
        };

        globals.set("start_x", start_x).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        globals.set("end_x", end_x).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        globals.set("start_y", start_y).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        globals.set("end_y", end_y).map_err(|error| anyhow::anyhow!(error.to_string()))?;

        // Begin atomic undo
        {
            let mut screen_guard = screen.lock();
            let edit_state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .ok_or_else(|| anyhow::anyhow!("Screen is not EditState"))?;
            let _ = edit_state.begin_atomic_undo(fl!(LANGUAGE_LOADER, "undo-plugin", title = self.title.clone()));
        }

        lua.load(&self.text).exec().map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(())
    }

    /// Read all plugins from the plugin directory
    pub fn read_plugin_directory() -> Vec<Self> {
        let mut result = Vec::new();
        let Some(root) = Settings::plugin_dir() else {
            log::error!("Can't get plugin directory.");
            return result;
        };

        // Create directory and copy default plugins if it doesn't exist
        if !root.exists() {
            log::info!("Creating plugin directory: {root:?}");
            if fs::create_dir_all(&root).is_err() {
                log::error!("Can't create plugin directory: {root:?}");
                return result;
            }
            // Copy default plugins
            Self::install_default_plugins(&root);
        }

        let walker = WalkDir::new(root).into_iter();
        for entry in walker.filter_entry(|e| !is_hidden(e)) {
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

    /// Install default plugins to the plugin directory
    fn install_default_plugins(dir: &Path) {
        let plugins = [
            ("random-colors.lua", include_bytes!("../../external/plugins/random-colors.lua").as_slice()),
            ("matrix_pattern.lua", include_bytes!("../../external/plugins/matrix_pattern.lua").as_slice()),
            ("rainbow_gradient.lua", include_bytes!("../../external/plugins/rainbow_gradient.lua").as_slice()),
            (
                "color_transformer.lua",
                include_bytes!("../../external/plugins/color_transformer.lua").as_slice(),
            ),
            ("lower_intensity.lua", include_bytes!("../../external/plugins/lower_intensity.lua").as_slice()),
            (
                "vertical_gradient.lua",
                include_bytes!("../../external/plugins/vertical_gradient.lua").as_slice(),
            ),
            ("random_mandala.lua", include_bytes!("../../external/plugins/random_mandala.lua").as_slice()),
            (
                "horizontal_gradient.lua",
                include_bytes!("../../external/plugins/horizontal_gradient.lua").as_slice(),
            ),
            (
                "double_line_frame.lua",
                include_bytes!("../../external/plugins/double_line_frame.lua").as_slice(),
            ),
            ("radial_gradient.lua", include_bytes!("../../external/plugins/radial_gradient.lua").as_slice()),
            (
                "horizontal_stripes.lua",
                include_bytes!("../../external/plugins/horizontal_stripes.lua").as_slice(),
            ),
            ("random_blocks.lua", include_bytes!("../../external/plugins/random_blocks.lua").as_slice()),
            ("grid_pattern.lua", include_bytes!("../../external/plugins/grid_pattern.lua").as_slice()),
            ("vertical_stripes.lua", include_bytes!("../../external/plugins/vertical_stripes.lua").as_slice()),
            ("shadow_effect.lua", include_bytes!("../../external/plugins/shadow_effect.lua").as_slice()),
            (
                "grayscale_gradient.lua",
                include_bytes!("../../external/plugins/grayscale_gradient.lua").as_slice(),
            ),
            (
                "increase_intensity.lua",
                include_bytes!("../../external/plugins/increase_intensity.lua").as_slice(),
            ),
        ];

        for (name, content) in plugins {
            if let Err(err) = fs::write(dir.join(name), content) {
                log::error!("Error writing plugin {name}: {err}");
            }
        }
    }

    /// Group plugins by their menu path for building hierarchical menus
    pub fn group_by_path(plugins: &[Plugin]) -> Vec<(String, Vec<(usize, &Plugin)>)> {
        let mut buttons: HashMap<String, Vec<(usize, &Plugin)>> = HashMap::new();

        for (i, p) in plugins.iter().enumerate() {
            let path = if p.path.is_empty() { String::new() } else { p.path[0].clone() };
            buttons.entry(path).or_default().push((i, p));
        }

        let mut result: Vec<_> = buttons.into_iter().collect();
        result.sort_by(|a, b| {
            if a.0.is_empty() {
                return std::cmp::Ordering::Greater;
            }
            if b.0.is_empty() {
                return std::cmp::Ordering::Less;
            }
            a.0.cmp(&b.0)
        });

        for (_i, v) in result.iter_mut() {
            v.sort_by(|a, b| a.1.title.cmp(&b.1.title));
        }

        result
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_str().map(|s| s.starts_with('.')).unwrap_or(false)
}

/// Lua wrapper for buffer access
struct LuaBufferView {
    screen: Arc<Mutex<Box<dyn icy_engine::Screen>>>,
}

impl LuaBufferView {
    fn with_edit_state<T, F: FnOnce(&mut EditState) -> T>(&self, f: F) -> mlua::Result<T> {
        let mut screen_guard = self.screen.lock();
        let edit_state = screen_guard
            .as_any_mut()
            .downcast_mut::<EditState>()
            .ok_or_else(|| mlua::Error::RuntimeError("Screen is not EditState".to_string()))?;
        Ok(f(edit_state))
    }

    fn convert_from_unicode(&self, ch: String) -> mlua::Result<char> {
        let Some(ch) = ch.chars().next() else {
            return Err(mlua::Error::SyntaxError {
                message: "Empty string".to_string(),
                incomplete_input: false,
            });
        };
        // For now, just pass through the character directly.
        // Buffer type-specific conversion could be added later if needed.
        Ok(ch)
    }

    fn convert_to_unicode(&self, ch: AttributedChar) -> String {
        // For now, just return the character directly as a string.
        // Buffer type-specific conversion could be added later if needed.
        ch.ch.to_string()
    }
}

impl UserData for LuaBufferView {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("height", |_, this| this.with_edit_state(|state| state.get_buffer().height()));

        fields.add_field_method_set("height", |_, this, val| {
            this.with_edit_state(|state| state.with_buffer_mut_no_undo(|buf| buf.set_height(val)))
        });

        fields.add_field_method_get("width", |_, this| this.with_edit_state(|state| state.get_buffer().width()));

        fields.add_field_method_set("width", |_, this, val| {
            this.with_edit_state(|state| state.with_buffer_mut_no_undo(|buf| buf.set_width(val)))
        });

        fields.add_field_method_get("font_page", |_, this| this.with_edit_state(|state| state.get_caret().font_page()));

        fields.add_field_method_set("font_page", |_, this, val| this.with_edit_state(|state| state.set_caret_font_page(val)));

        fields.add_field_method_get("layer", |_, this| this.with_edit_state(|state| state.get_current_layer().unwrap_or(0)));

        fields.add_field_method_set("layer", |_, this, val: usize| {
            this.with_edit_state(|state| {
                if val < state.get_buffer().layers.len() {
                    state.set_current_layer(val);
                    Ok(())
                } else {
                    Err(mlua::Error::SyntaxError {
                        message: format!("Layer {} out of range (0..<{})", val, state.get_buffer().layers.len()),
                        incomplete_input: false,
                    })
                }
            })?
        });

        fields.add_field_method_get("fg", |_, this| this.with_edit_state(|state| state.get_caret().attribute.foreground()));

        fields.add_field_method_set("fg", |_, this, val| this.with_edit_state(|state| state.set_caret_foreground(val)));

        fields.add_field_method_get("bg", |_, this| this.with_edit_state(|state| state.get_caret().attribute.background()));

        fields.add_field_method_set("bg", |_, this, val| this.with_edit_state(|state| state.set_caret_background(val)));

        fields.add_field_method_get("x", |_, this| this.with_edit_state(|state| state.get_caret().position().x));

        fields.add_field_method_set("x", |_, this, val| this.with_edit_state(|state| state.set_caret_x(val)));

        fields.add_field_method_get("y", |_, this| this.with_edit_state(|state| state.get_caret().position().y));

        fields.add_field_method_set("y", |_, this, val| this.with_edit_state(|state| state.set_caret_y(val)));

        fields.add_field_method_get("layer_count", |_, this| this.with_edit_state(|state| state.get_buffer().layers.len()));
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("fg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            this.with_edit_state(|state| {
                let color = state.with_buffer_mut_no_undo(|buf| buf.palette.insert_color_rgb(r, g, b));
                state.set_caret_foreground(color);
                color
            })
        });

        methods.add_method_mut("bg_rgb", |_, this, (r, g, b): (u8, u8, u8)| {
            this.with_edit_state(|state| {
                let color = state.with_buffer_mut_no_undo(|buf| buf.palette.insert_color_rgb(r, g, b));
                state.set_caret_background(color);
                color
            })
        });

        methods.add_method_mut("set_char", |_, this, (x, y, ch): (i32, i32, String)| {
            let ch_converted = this.convert_from_unicode(ch)?;
            this.with_edit_state(|state| {
                let cur_layer = state.get_current_layer().unwrap_or(0);
                let layer_len = state.get_buffer().layers.len();
                if cur_layer >= layer_len {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                        incomplete_input: false,
                    });
                }
                let mut attr = state.get_caret().attribute.clone();
                attr.attr &= !attribute::INVISIBLE;
                let ch = AttributedChar::new(ch_converted, attr);

                if let Err(err) = state.set_char((x, y), ch) {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Error setting char: {}", err),
                        incomplete_input: false,
                    });
                };
                Ok(())
            })?
        });

        methods.add_method_mut("get_char", |_, this, (x, y): (i32, i32)| {
            let ch = this.with_edit_state(|state| {
                let cur_layer = state.get_current_layer().unwrap_or(0);
                let layer_len = state.get_buffer().layers.len();
                if cur_layer >= layer_len {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                        incomplete_input: false,
                    });
                }
                Ok(state.get_buffer().layers[cur_layer].char_at(Position::new(x, y)))
            })??;
            Ok(this.convert_to_unicode(ch))
        });

        methods.add_method_mut("grab_char", |_, this, (x, y): (i32, i32)| {
            let ch = this.with_edit_state(|state| {
                let cur_layer = state.get_current_layer().unwrap_or(0);
                let layer_len = state.get_buffer().layers.len();
                if cur_layer >= layer_len {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                        incomplete_input: false,
                    });
                }

                let ch = state.get_buffer().layers[cur_layer].char_at(Position::new(x, y));
                // Set caret colors from the grabbed character
                state.set_caret_foreground(ch.attribute.foreground());
                state.set_caret_background(ch.attribute.background());

                Ok(ch)
            })??;
            Ok(this.convert_to_unicode(ch))
        });

        methods.add_method_mut("set_fg", |_, this, (x, y, col): (i32, i32, u32)| {
            this.with_edit_state(|state| {
                let cur_layer = state.get_current_layer().unwrap_or(0);
                let layer_len = state.get_buffer().layers.len();
                if cur_layer >= layer_len {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                        incomplete_input: false,
                    });
                }
                let mut ch = state.get_buffer().layers[cur_layer].char_at(Position::new(x, y));
                ch.attribute.set_foreground(col);
                if let Err(err) = state.set_char((x, y), ch) {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Error setting char: {}", err),
                        incomplete_input: false,
                    });
                }
                Ok(())
            })?
        });

        methods.add_method_mut("get_fg", |_, this, (x, y): (i32, i32)| {
            this.with_edit_state(|state| {
                let cur_layer = state.get_current_layer().unwrap_or(0);
                let layer_len = state.get_buffer().layers.len();
                if cur_layer >= layer_len {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                        incomplete_input: false,
                    });
                }
                let ch = state.get_buffer().layers[cur_layer].char_at(Position::new(x, y));
                Ok(ch.attribute.foreground())
            })?
        });

        methods.add_method_mut("set_bg", |_, this, (x, y, col): (i32, i32, u32)| {
            this.with_edit_state(|state| {
                let cur_layer = state.get_current_layer().unwrap_or(0);
                let layer_len = state.get_buffer().layers.len();
                if cur_layer >= layer_len {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                        incomplete_input: false,
                    });
                }
                let mut ch = state.get_buffer().layers[cur_layer].char_at(Position::new(x, y));
                ch.attribute.set_background(col);
                if let Err(err) = state.set_char((x, y), ch) {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Error setting char: {}", err),
                        incomplete_input: false,
                    });
                }
                Ok(())
            })?
        });

        methods.add_method_mut("get_bg", |_, this, (x, y): (i32, i32)| {
            this.with_edit_state(|state| {
                let cur_layer = state.get_current_layer().unwrap_or(0);
                let layer_len = state.get_buffer().layers.len();
                if cur_layer >= layer_len {
                    return Err(mlua::Error::SyntaxError {
                        message: format!("Current layer {} out of range (0..<{})", cur_layer, layer_len),
                        incomplete_input: false,
                    });
                }
                let ch = state.get_buffer().layers[cur_layer].char_at(Position::new(x, y));
                Ok(ch.attribute.background())
            })?
        });

        methods.add_method_mut("print", |_, this, str: String| {
            for c in str.chars() {
                let ch_converted = this.convert_from_unicode(c.to_string())?;
                this.with_edit_state(|state| {
                    let pos = state.get_caret().position();
                    let mut attribute = state.get_caret().attribute.clone();
                    attribute.attr &= !attribute::INVISIBLE;
                    let ch = AttributedChar::new(ch_converted, attribute);
                    let _ = state.set_char(pos, ch);
                    state.set_caret_position(Position::new(pos.x + 1, pos.y));
                })?;
            }
            Ok(())
        });

        methods.add_method_mut("gotoxy", |_, this, (x, y): (i32, i32)| {
            this.with_edit_state(|state| {
                state.set_caret_position(Position::new(x, y));
            })
        });

        methods.add_method_mut("set_layer_position", |_, this, (layer, x, y): (usize, i32, i32)| {
            this.with_edit_state(|state| {
                if layer < state.get_buffer().layers.len() {
                    let _ = state.move_layer(Position::new(x, y));
                    Ok(())
                } else {
                    Err(mlua::Error::SyntaxError {
                        message: format!("Layer {} out of range (0..<{})", layer, state.get_buffer().layers.len()),
                        incomplete_input: false,
                    })
                }
            })?
        });

        methods.add_method_mut("get_layer_position", |_, this, layer: usize| {
            this.with_edit_state(|state| {
                if layer < state.get_buffer().layers.len() {
                    let pos = state.get_buffer().layers[layer].offset();
                    Ok((pos.x, pos.y))
                } else {
                    Err(mlua::Error::SyntaxError {
                        message: format!("Layer {} out of range (0..<{})", layer, state.get_buffer().layers.len()),
                        incomplete_input: false,
                    })
                }
            })?
        });

        methods.add_method_mut("set_layer_visible", |_, this, (layer, is_visible): (i32, bool)| {
            let layer = layer as usize;
            this.with_edit_state(|state| {
                let layer_len = state.get_buffer().layers.len();
                if layer < layer_len {
                    state.with_buffer_mut_no_undo(|buf| buf.layers[layer].set_is_visible(is_visible));
                    Ok(())
                } else {
                    Err(mlua::Error::SyntaxError {
                        message: format!("Layer {} out of range (0..<{})", layer, layer_len),
                        incomplete_input: false,
                    })
                }
            })?
        });

        methods.add_method_mut("get_layer_visible", |_, this, layer: usize| {
            this.with_edit_state(|state| {
                if layer < state.get_buffer().layers.len() {
                    Ok(state.get_buffer().layers[layer].is_visible())
                } else {
                    Err(mlua::Error::SyntaxError {
                        message: format!("Layer {} out of range (0..<{})", layer, state.get_buffer().layers.len()),
                        incomplete_input: false,
                    })
                }
            })?
        });

        methods.add_method_mut("clear", |_, this, ()| {
            this.with_edit_state(|state| {
                state.with_buffer_mut_no_undo(|buf| buf.reset_terminal());
            })
        });
    }
}
