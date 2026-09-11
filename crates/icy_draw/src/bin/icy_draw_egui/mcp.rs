use super::{Dialog, DrawApp};
use eframe::egui;
use icy_draw::{
    document::Document,
    mcp::{edit::respond, types::*, McpCommand, McpServer},
};
use icy_engine::Size;
use icy_engine_edit::bitfont::BitFontUndoState;
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};

pub struct Bridge {
    pub events: mpsc::Receiver<Result<McpCommand, String>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Bridge {
    pub fn start(port: u16, context: egui::Context) -> Self {
        let (sender, events) = mpsc::channel();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = sender.send(Err(error.to_string()));
                    context.request_repaint();
                    return;
                }
            };
            runtime.block_on(async {
                let (server, mut commands) = McpServer::new();
                let forward = async {
                    while let Some(command) = commands.recv().await { if sender.send(Ok(command)).is_err() { break; } context.request_repaint(); }
                };
                tokio::select! {
                    result = Arc::new(server).start(port) => { if let Err(error) = result { let _ = sender.send(Err(error.to_string())); context.request_repaint(); } },
                    _ = forward => {},
                    _ = stopped => {},
                }
            });
        });
        Self { events, stop: Some(stop) }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl DrawApp {
    pub fn enable_mcp(&mut self, port: u16, context: egui::Context) {
        self.mcp = Some(Bridge::start(port, context));
    }

    pub(super) fn mcp_command(&mut self, command: McpCommand) {
        let unavailable = if self.picker {
            Some("A native file dialog is open")
        } else if self.animation.is_some() || self.font_editor.is_some() {
            Some("Not in ANSI editor mode")
        } else if self.dialog.is_some() {
            Some("Close the editor dialog before using ANSI automation")
        } else {
            None
        };
        if icy_draw::mcp::edit::handle(&mut self.document, &command, unavailable) {
            return;
        }
        let busy = self.picker || self.dialog.as_ref().is_some_and(|dialog| !matches!(dialog, Dialog::Font));
        match command {
            McpCommand::GetHelp { editor_type, response } => respond(
                &response,
                match editor_type.as_deref() {
                    Some("ansi") => include_str!("../../../doc/ANSI.md"),
                    Some("animation") => include_str!("../../../doc/ANIMATION.md"),
                    Some("bitfont") => include_str!("../../../doc/BITFONT.md"),
                    _ => include_str!("../../../doc/HELP.md"),
                }
                .to_owned(),
            ),
            McpCommand::GetStatus(response) => {
                let mut status = EditorStatus {
                    editor: if self.charfont.is_some() { "charfont".into() } else { "ansi".into() },
                    file: self
                        .charfont
                        .as_ref()
                        .and_then(|font| font.path.as_ref())
                        .or(self.document.path.as_ref())
                        .map(|path| path.display().to_string()),
                    dirty: self.modified(),
                    ansi: Some(
                        self.document
                            .with_state(|state| icy_draw::mcp::edit::status(state, self.settings.font_outline_style)),
                    ),
                    animation: None,
                    bitfont: None,
                };
                if let Some(editor) = &self.animation {
                    status.editor = "animation".into();
                    status.ansi = None;
                    status.animation = Some(editor.mcp_status());
                    status.file = editor.path.as_ref().map(|path| path.display().to_string());
                }
                if let Some(editor) = &self.font_editor {
                    status.editor = "bitfont".into();
                    status.ansi = None;
                    status.bitfont = Some(editor.mcp_status());
                    status.dirty = editor.modified();
                    status.file = editor.path.as_ref().map(|path| path.display().to_string());
                }
                respond(&response, status);
            }
            McpCommand::NewDocument { doc_type, response } => {
                if !busy {
                    self.document.finish();
                    if let Some(editor) = &mut self.font_editor {
                        editor.finish();
                    }
                }
                let result = if busy || self.modified() || self.font_editor.as_ref().is_some_and(|editor| editor.modified()) {
                    Err("Save or close the current document first".into())
                } else {
                    match doc_type.as_str() {
                        "ansi" => {
                            self.replace(Document::new(Size::new(80, 25)));
                            Ok(())
                        }
                        "animation" => {
                            self.replace(Document::new(Size::new(80, 25)));
                            self.animation = Some(crate::animation::AnimationEditor::new());
                            Ok(())
                        }
                        "charfont" => {
                            let font = icy_draw::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
                            self.replace(font.document());
                            self.charfont = Some(font);
                            Ok(())
                        }
                        "bitfont" => {
                            self.font_editor = Some(crate::font::FontEditor::new(icy_engine::BitFont::create_8("Untitled", 8, 16, &[0; 4096])));
                            self.dialog = Some(Dialog::Font);
                            Ok(())
                        }
                        _ => Err("Unknown editor type".into()),
                    }
                };
                respond(&response, result);
            }
            McpCommand::LoadDocument { path, response } => {
                if !busy {
                    self.document.finish();
                    if let Some(editor) = &mut self.font_editor {
                        editor.finish();
                    }
                }
                let result = if busy || self.modified() || self.font_editor.as_ref().is_some_and(|editor| editor.modified()) {
                    Err("Save or close the current document first".into())
                } else {
                    self.open(PathBuf::from(path));
                    if let Some(Dialog::Error(error)) = &self.dialog {
                        Err(error.clone())
                    } else {
                        Ok(())
                    }
                };
                respond(&response, result);
            }
            McpCommand::Save(response) => {
                let result = if busy {
                    Err("Close the current dialog first".into())
                } else if let Some(editor) = &mut self.font_editor {
                    editor.path.clone().ok_or("No file path set".into()).and_then(|path| editor.save(&path, false))
                } else if let Some(editor) = &mut self.animation {
                    editor.path.clone().ok_or("No file path set".into()).and_then(|path| editor.save(&path, false))
                } else if let Some(font) = &mut self.charfont {
                    font.path
                        .clone()
                        .ok_or("No file path set".into())
                        .and_then(|path| font.save(&mut self.document, &path))
                } else {
                    self.document
                        .path
                        .clone()
                        .ok_or("No file path set".into())
                        .and_then(|path| self.document.save(&path, false))
                };
                respond(&response, result);
            }
            McpCommand::Undo(ref response) | McpCommand::Redo(ref response) => {
                let redo = matches!(command, McpCommand::Redo(_));
                let result = if busy {
                    Err("Close the current dialog first".into())
                } else if let Some(editor) = &mut self.font_editor {
                    editor.finish();
                    (if redo { editor.state.redo() } else { editor.state.undo() }).map_err(|error| error.to_string())
                } else if let Some(editor) = &mut self.animation {
                    editor.undo_source(redo);
                    Ok(())
                } else {
                    self.undo(redo);
                    if let Some(Dialog::Error(error)) = &self.dialog {
                        Err(error.clone())
                    } else {
                        Ok(())
                    }
                };
                respond(response, result);
            }
            McpCommand::AnimationGetText { offset, length, response } => {
                let result = self.animation.as_ref().ok_or("Not in animation editor mode".into()).and_then(|editor| {
                    let start = offset.unwrap_or(0);
                    let end = start
                        .saturating_add(length.unwrap_or(editor.source.len().saturating_sub(start)))
                        .min(editor.source.len());
                    editor.source.get(start..end).map(str::to_owned).ok_or("Invalid UTF-8 byte range".into())
                });
                respond(&response, result);
            }
            McpCommand::AnimationReplaceText {
                offset,
                length,
                text,
                response,
            } => {
                let result = if busy {
                    Err("Close the current dialog first".into())
                } else {
                    self.animation
                        .as_mut()
                        .ok_or("Not in animation editor mode".into())
                        .and_then(|editor| editor.replace_text(offset, length, &text))
                };
                respond(&response, result);
            }
            McpCommand::AnimationGetScreen { frame, format, response } => respond(
                &response,
                self.animation
                    .as_ref()
                    .ok_or("Not in animation editor mode".into())
                    .and_then(|editor| editor.mcp_screen(frame, format)),
            ),
            McpCommand::BitFontListChars(response) => respond(
                &response,
                self.font_editor
                    .as_ref()
                    .ok_or("Not in bitmap font editor mode".into())
                    .map(|editor| (0..editor.state.get_all_glyph_data().len() as u32).collect()),
            ),
            McpCommand::BitFontGetChar { code, response } => respond(
                &response,
                self.font_editor
                    .as_ref()
                    .ok_or("Not in bitmap font editor mode".into())
                    .and_then(|editor| editor.mcp_get(code)),
            ),
            McpCommand::BitFontSetChar { code, data, response } => respond(
                &response,
                if busy {
                    Err("Close the current dialog first".into())
                } else {
                    self.font_editor
                        .as_mut()
                        .ok_or("Not in bitmap font editor mode".into())
                        .and_then(|editor| editor.mcp_set(code, &data))
                },
            ),
            _ => unreachable!("ANSI commands handled by document adapter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn response<T>() -> (icy_draw::mcp::SenderType<T>, tokio::sync::oneshot::Receiver<T>) {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        (Arc::new(parking_lot::Mutex::new(Some(sender))), receiver)
    }

    #[test]
    fn commands_edit_undo_and_preserve_dirty_documents() {
        let mut app = DrawApp::new();
        let (response, mut result) = response();
        app.mcp_command(McpCommand::AnsiRunScript {
            script: "buf:set_char(0, 0, 'A')".into(),
            undo_description: None,
            response,
        });
        result.try_recv().unwrap().unwrap();
        assert!(app.modified());
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::NewDocument {
            doc_type: "animation".into(),
            response,
        });
        assert!(result.try_recv().unwrap().is_err());
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::Undo(response));
        result.try_recv().unwrap().unwrap();
        assert!(!app.modified());
        app.picker = true;
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::AnsiRunScript {
            script: "buf:set_char(0, 0, 'B')".into(),
            undo_description: None,
            response,
        });
        assert!(result.try_recv().unwrap().is_err());
        assert!(!app.modified());
    }

    #[test]
    fn animation_byte_ranges_and_bitmap_roundtrip() {
        let mut app = DrawApp::new();
        app.animation = Some(crate::animation::AnimationEditor::new());
        app.animation.as_mut().unwrap().source = "a\u{00e9}z".into();
        let (response, mut result) = response();
        app.mcp_command(McpCommand::AnimationReplaceText {
            offset: 2,
            length: 1,
            text: "X".into(),
            response,
        });
        assert!(result.try_recv().unwrap().is_err());
        assert_eq!(app.animation.as_ref().unwrap().source, "a\u{00e9}z");
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::AnimationReplaceText {
            offset: 1,
            length: 2,
            text: "X".into(),
            response,
        });
        result.try_recv().unwrap().unwrap();
        app.animation.as_mut().unwrap().undo_source(false);
        assert_eq!(app.animation.as_ref().unwrap().source, "a\u{00e9}z");
        let mut font = crate::font::FontEditor::new(icy_engine::BitFont::create_8("Test", 7, 13, &[0; 3328]));
        font.state.set_pixel('A', 6, 12, true).unwrap();
        let data = font.mcp_get(65).unwrap();
        font.state.clear_glyph('A').unwrap();
        font.mcp_set(65, &data).unwrap();
        assert!(font.state.get_glyph_pixels('A')[12][6]);
        assert!(font.mcp_set(66, &data).is_err());
    }

    #[test]
    fn read_only_and_blocked_commands_preserve_pending_shape() {
        let mut app = DrawApp::new();
        app.document.tool = icy_engine_edit::tools::Tool::Line;
        app.document.begin((0, 0).into(), icy_engine::MouseButton::Left);
        app.document.update((5, 0).into());
        let preview = app.document.preview.clone();
        assert!(!preview.is_empty());
        let (response, mut result) = response();
        app.mcp_command(McpCommand::GetStatus(response));
        result.try_recv().unwrap();
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::AnsiGetCaret { response });
        result.try_recv().unwrap().unwrap();
        app.picker = true;
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::AnsiRunScript {
            script: "buf:set_char(0, 0, 'A')".into(),
            undo_description: None,
            response,
        });
        assert!(result.try_recv().unwrap().is_err());
        assert_eq!(app.document.preview, preview);
        app.picker = false;
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::NewDocument {
            doc_type: "ansi".into(),
            response,
        });
        assert!(result.try_recv().unwrap().is_err());
        assert!(app.document.preview.is_empty());
        assert!(app.modified());
    }

    #[test]
    fn clean_bitmap_mode_does_not_survive_document_replacement() {
        let mut app = DrawApp::new();
        let (response, mut result) = response();
        app.mcp_command(McpCommand::NewDocument {
            doc_type: "bitfont".into(),
            response,
        });
        result.try_recv().unwrap().unwrap();
        assert!(app.font_editor.is_some());
        let (response, mut result) = self::response();
        app.mcp_command(McpCommand::NewDocument {
            doc_type: "ansi".into(),
            response,
        });
        result.try_recv().unwrap().unwrap();
        assert!(app.font_editor.is_none());
        assert!(app.dialog.is_none());
    }

    #[test]
    fn opening_a_file_keeps_unsaved_bitmap_font_visible() {
        let mut app = DrawApp::new();
        let (response, mut result) = response();
        app.mcp_command(McpCommand::NewDocument {
            doc_type: "bitfont".into(),
            response,
        });
        result.try_recv().unwrap().unwrap();
        app.font_editor.as_mut().unwrap().state.set_pixel('A', 0, 0, true).unwrap();
        app.open(PathBuf::from("missing.icy"));
        assert!(matches!(app.dialog, Some(Dialog::Font)));
        assert!(app.font_editor.as_ref().unwrap().modified());
        assert!(app.font_editor.as_ref().unwrap().state.get_glyph_pixels('A')[0][0]);
    }

    #[test]
    fn http_initialize_uses_existing_mcp_server() {
        use std::io::{Read, Write};
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let bridge = Bridge::start(address.port(), egui::Context::default());
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            if let Ok(socket) = std::net::TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
                break socket;
            }
            assert!(Instant::now() < deadline, "MCP listener did not start");
            std::thread::yield_now();
        };
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"draw-test","version":"1"}}}).to_string();
        write!(socket, "POST / HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut output = String::new();
        socket.read_to_string(&mut output).unwrap();
        assert!(output.starts_with("HTTP/1.1 200"), "{output}");
        assert!(output.contains("serverInfo"), "{output}");
        drop(bridge);
    }
}
