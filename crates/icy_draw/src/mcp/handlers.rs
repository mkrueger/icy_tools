//! MCP tool handlers for icy_draw

use crate::mcp::McpCommand;
use crate::mcp::types::{
    AnimationGetScreenRequest, AnimationGetTextRequest, AnimationReplaceTextRequest, AnsiAddLayerRequest, AnsiDeleteLayerRequest, AnsiGetLayerRequest,
    AnsiGetRegionRequest, AnsiGetScreenRequest, AnsiMergeDownLayerRequest, AnsiMoveLayerRequest, AnsiResizeRequest, AnsiRunScriptRequest,
    AnsiSelectionActionRequest, AnsiSetCaretRequest, AnsiSetCharRequest, AnsiSetColorRequest, AnsiSetLayerPropsRequest, AnsiSetRegionRequest,
    AnsiSetSelectionRequest, BitFontGetCharRequest, BitFontSetCharRequest, CharListResponse, EditorStatus, GetHelpRequest, LoadDocumentRequest,
    NewDocumentRequest,
};

use parking_lot::Mutex;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{
        tool::{ToolCallContext, ToolRouter},
        wrapper::Parameters,
    },
    model::{
        CallToolRequestParam, CallToolResult, Content, Implementation, InitializeResult, ListToolsResult, PaginatedRequestParam, ProtocolVersion,
        ServerCapabilities,
    },
    tool, tool_router,
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

/// Embedded documentation files
const HELP_DOC: &str = include_str!("../../doc/HELP.md");
const ANIMATION_DOC: &str = include_str!("../../doc/ANIMATION.md");
const BITFONT_DOC: &str = include_str!("../../doc/BITFONT.md");
const ANSI_DOC: &str = include_str!("../../doc/ANSI.md");

/// Timeout for commands that need a response
const COMMAND_TIMEOUT_SECS: u64 = 30;

#[derive(Clone)]
pub struct IcyDrawMcpHandler {
    command_tx: mpsc::UnboundedSender<McpCommand>,
    pub tool_router: ToolRouter<Self>,
}

#[tool_router]
impl IcyDrawMcpHandler {
    pub fn new(command_tx: mpsc::UnboundedSender<McpCommand>) -> Self {
        Self {
            command_tx,
            tool_router: Self::tool_router(),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // General tools
    // ═══════════════════════════════════════════════════════════════════════

    #[tool(
        description = "Get documentation. Without 'editor' parameter: general overview. With 'ansi', 'animation' or 'bitfont': editor-specific documentation."
    )]
    async fn get_help(&self, params: Parameters<GetHelpRequest>) -> Result<CallToolResult, McpError> {
        let doc = match params.0.editor.as_deref() {
            Some("ansi") => ANSI_DOC,
            Some("animation") => ANIMATION_DOC,
            Some("bitfont") => BITFONT_DOC,
            _ => HELP_DOC,
        };
        Ok(CallToolResult::success(vec![Content::text(doc)]))
    }

    #[tool(description = "Get current editor status including mode, file path, dimensions, and editor-specific information.")]
    async fn get_status(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::GetStatus(Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let status: EditorStatus = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout waiting for status", None))?
            .map_err(|_| McpError::internal_error("Failed to get status", None))?;

        let json = serde_json::to_string_pretty(&status).map_err(|e| McpError::internal_error(format!("Failed to serialize status: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Create a new document. Types: 'ansi', 'animation', 'bitfont', 'charfont'")]
    async fn new_document(&self, params: Parameters<NewDocumentRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::NewDocument {
                doc_type: params.0.doc_type.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout creating document", None))?
            .map_err(|_| McpError::internal_error("Failed to create document", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Created new {} document",
                params.0.doc_type
            ))])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Open a file from the specified path.")]
    async fn load_document(&self, params: Parameters<LoadDocumentRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::LoadDocument {
                path: params.0.path.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout loading document", None))?
            .map_err(|_| McpError::internal_error("Failed to load document", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(format!("Loaded {}", params.0.path))])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Save the current document.")]
    async fn save(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::Save(Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout saving", None))?
            .map_err(|_| McpError::internal_error("Failed to save", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Document saved")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Undo the last action.")]
    async fn undo(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::Undo(Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to undo", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Undone")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Redo the previously undone action.")]
    async fn redo(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::Redo(Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to redo", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Redone")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Animation editor tools
    // ═══════════════════════════════════════════════════════════════════════

    #[tool(description = "[Animation Editor] Get Lua script text. Without parameters returns entire script.")]
    async fn animation_get_text(&self, params: Parameters<AnimationGetTextRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnimationGetText {
                offset: params.0.offset,
                length: params.0.length,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to get text", None))?;

        match result {
            Ok(text) => Ok(CallToolResult::success(vec![Content::text(text)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[Animation Editor] Replace text in Lua script at byte offset.")]
    async fn animation_replace_text(&self, params: Parameters<AnimationReplaceTextRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnimationReplaceText {
                offset: params.0.offset,
                length: params.0.length,
                text: params.0.text.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to replace text", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Text replaced")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[Animation Editor] Get rendered frame as ANSI text. Frame numbers are 1-based.")]
    async fn animation_get_screen(&self, params: Parameters<AnimationGetScreenRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnimationGetScreen {
                frame: params.0.frame,
                format: params.0.format.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to get screen", None))?;

        match result {
            Ok(screen) => Ok(CallToolResult::success(vec![Content::text(screen)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // BitFont editor tools
    // ═══════════════════════════════════════════════════════════════════════

    #[tool(description = "[BitFont Editor] List all character codes in the font.")]
    async fn bitfont_list_chars(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::BitFontListChars(Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to list chars", None))?;

        match result {
            Ok(chars) => {
                let response = CharListResponse { count: chars.len(), chars };
                let json = serde_json::to_string_pretty(&response).map_err(|e| McpError::internal_error(format!("Failed to serialize: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[BitFont Editor] Get glyph bitmap as base64-encoded data.")]
    async fn bitfont_get_char(&self, params: Parameters<BitFontGetCharRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::BitFontGetChar {
                code: params.0.code,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to get char", None))?;

        match result {
            Ok(glyph) => {
                let json = serde_json::to_string_pretty(&glyph).map_err(|e| McpError::internal_error(format!("Failed to serialize: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[BitFont Editor] Set glyph bitmap from base64-encoded data.")]
    async fn bitfont_set_char(&self, params: Parameters<BitFontSetCharRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::BitFontSetChar {
                code: params.0.code,
                data: params.0.data.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout", None))?
            .map_err(|_| McpError::internal_error("Failed to set char", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Glyph updated")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // ANSI editor tools
    // ═══════════════════════════════════════════════════════════════════════

    #[tool(
        description = "[ANSI Editor] Run a Lua script on the current buffer. The script has access to the 'buf' object with all buffer manipulation methods (set_char, get_char, fg_rgb, bg_rgb, print, etc.) and selection bounds (start_x, end_x, start_y, end_y). Changes are wrapped in an atomic undo operation."
    )]
    async fn ansi_run_script(&self, params: Parameters<AnsiRunScriptRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiRunScript {
                script: params.0.script.clone(),
                undo_description: params.0.undo_description.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout executing script", None))?
            .map_err(|_| McpError::internal_error("Failed to execute script", None))?;

        match result {
            Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(
        description = "[ANSI Editor] Get full layer data including all character cells. Returns layer properties and a flat array of character data (row-major order)."
    )]
    async fn ansi_get_layer(&self, params: Parameters<AnsiGetLayerRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiGetLayer {
                layer: params.0.layer,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout getting layer", None))?
            .map_err(|_| McpError::internal_error("Failed to get layer", None))?;

        match result {
            Ok(layer_data) => {
                let json = serde_json::to_string_pretty(&layer_data).map_err(|e| McpError::internal_error(format!("Failed to serialize: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Set a character at a specific position in a layer with the given text attribute.")]
    async fn ansi_set_char(&self, params: Parameters<AnsiSetCharRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiSetChar {
                layer: params.0.layer,
                x: params.0.x,
                y: params.0.y,
                ch: params.0.ch.clone(),
                attribute: params.0.attribute.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout setting char", None))?
            .map_err(|_| McpError::internal_error("Failed to set char", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Character set")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Set a palette color by index. Changes the RGB values of a specific palette entry.")]
    async fn ansi_set_color(&self, params: Parameters<AnsiSetColorRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiSetColor {
                index: params.0.index,
                r: params.0.r,
                g: params.0.g,
                b: params.0.b,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout setting color", None))?
            .map_err(|_| McpError::internal_error("Failed to set color", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(format!("Palette color {} set", params.0.index))])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Get the current screen. format: 'ansi' (with escape codes) or 'ascii' (plain text, no images).")]
    async fn ansi_get_screen(&self, params: Parameters<AnsiGetScreenRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiGetScreen {
                format: params.0.format.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout getting screen", None))?
            .map_err(|_| McpError::internal_error("Failed to get screen", None))?;

        match result {
            Ok(screen) => Ok(CallToolResult::success(vec![Content::text(screen)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Get current caret x/y and text attribute.")]
    async fn ansi_get_caret(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiGetCaret {
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout getting caret", None))?
            .map_err(|_| McpError::internal_error("Failed to get caret", None))?;

        match result {
            Ok(caret) => {
                let json = serde_json::to_string_pretty(&caret).map_err(|e| McpError::internal_error(format!("Failed to serialize: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Set caret x/y and text attribute.")]
    async fn ansi_set_caret(&self, params: Parameters<AnsiSetCaretRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiSetCaret {
                x: params.0.x,
                y: params.0.y,
                attribute: params.0.attribute.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout setting caret", None))?
            .map_err(|_| McpError::internal_error("Failed to set caret", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Caret set")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] List all layers (metadata only).")]
    async fn ansi_list_layers(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiListLayers {
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout listing layers", None))?
            .map_err(|_| McpError::internal_error("Failed to list layers", None))?;

        match result {
            Ok(layers) => {
                let json = serde_json::to_string_pretty(&layers).map_err(|e| McpError::internal_error(format!("Failed to serialize: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Add a new layer after a given layer index.")]
    async fn ansi_add_layer(&self, params: Parameters<AnsiAddLayerRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiAddLayer {
                after_layer: params.0.after_layer,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout adding layer", None))?
            .map_err(|_| McpError::internal_error("Failed to add layer", None))?;

        match result {
            Ok(idx) => Ok(CallToolResult::success(vec![Content::text(format!("Layer added: {}", idx))])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Delete a layer by index.")]
    async fn ansi_delete_layer(&self, params: Parameters<AnsiDeleteLayerRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiDeleteLayer {
                layer: params.0.layer,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout deleting layer", None))?
            .map_err(|_| McpError::internal_error("Failed to delete layer", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Layer deleted")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Update layer properties (title/visibility/locks/offset/transparency).")]
    async fn ansi_set_layer_props(&self, params: Parameters<AnsiSetLayerPropsRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiSetLayerProps {
                layer: params.0.layer,
                title: params.0.title.clone(),
                is_visible: params.0.is_visible,
                is_locked: params.0.is_locked,
                is_position_locked: params.0.is_position_locked,
                offset_x: params.0.offset_x,
                offset_y: params.0.offset_y,
                transparency: params.0.transparency,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout setting layer props", None))?
            .map_err(|_| McpError::internal_error("Failed to set layer props", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Layer properties updated")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Merge a layer down into the layer below.")]
    async fn ansi_merge_down_layer(&self, params: Parameters<AnsiMergeDownLayerRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiMergeDownLayer {
                layer: params.0.layer,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout merging layer", None))?
            .map_err(|_| McpError::internal_error("Failed to merge layer", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Layer merged down")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Move a layer up/down in the layer stack.")]
    async fn ansi_move_layer(&self, params: Parameters<AnsiMoveLayerRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiMoveLayer {
                layer: params.0.layer,
                direction: params.0.direction.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout moving layer", None))?
            .map_err(|_| McpError::internal_error("Failed to move layer", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Layer moved")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Resize the buffer to width/height.")]
    async fn ansi_resize(&self, params: Parameters<AnsiResizeRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiResize {
                width: params.0.width,
                height: params.0.height,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout resizing buffer", None))?
            .map_err(|_| McpError::internal_error("Failed to resize buffer", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Buffer resized")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Get a rectangular region from a layer (row-major CharInfo array).")]
    async fn ansi_get_region(&self, params: Parameters<AnsiGetRegionRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiGetRegion {
                layer: params.0.layer,
                x: params.0.x,
                y: params.0.y,
                width: params.0.width,
                height: params.0.height,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout getting region", None))?
            .map_err(|_| McpError::internal_error("Failed to get region", None))?;

        match result {
            Ok(region) => {
                let json = serde_json::to_string_pretty(&region).map_err(|e| McpError::internal_error(format!("Failed to serialize: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Set a rectangular region on a layer using a row-major CharInfo array.")]
    async fn ansi_set_region(&self, params: Parameters<AnsiSetRegionRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiSetRegion {
                layer: params.0.layer,
                x: params.0.x,
                y: params.0.y,
                width: params.0.width,
                height: params.0.height,
                chars: params.0.chars.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout setting region", None))?
            .map_err(|_| McpError::internal_error("Failed to set region", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Region set")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Get current selection (or null if none).")]
    async fn ansi_get_selection(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiGetSelection {
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout getting selection", None))?
            .map_err(|_| McpError::internal_error("Failed to get selection", None))?;

        match result {
            Ok(sel) => {
                let json = serde_json::to_string_pretty(&sel).map_err(|e| McpError::internal_error(format!("Failed to serialize: {e}"), None))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Set selection rectangle.")]
    async fn ansi_set_selection(&self, params: Parameters<AnsiSetSelectionRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiSetSelection {
                x: params.0.x,
                y: params.0.y,
                width: params.0.width,
                height: params.0.height,
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout setting selection", None))?
            .map_err(|_| McpError::internal_error("Failed to set selection", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Selection set")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Clear selection.")]
    async fn ansi_clear_selection(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiClearSelection {
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout clearing selection", None))?
            .map_err(|_| McpError::internal_error("Failed to clear selection", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Selection cleared")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "[ANSI Editor] Run a selection action (flip/justify/crop/etc).")]
    async fn ansi_selection_action(&self, params: Parameters<AnsiSelectionActionRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::AnsiSelectionAction {
                action: params.0.action.clone(),
                response: Arc::new(Mutex::new(Some(response_tx))),
            })
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Timeout running action", None))?
            .map_err(|_| McpError::internal_error("Failed to run action", None))?;

        match result {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text("Action executed")])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }
}

impl ServerHandler for IcyDrawMcpHandler {
    fn get_info(&self) -> InitializeResult {
        println!("[MCP DEBUG] get_info called - returning server capabilities");
        let result = InitializeResult {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "icy_draw_mcp".to_string(),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some("icy_draw MCP server for ANSI/ASCII art editing automation".to_string()),
        };
        println!("[MCP DEBUG] get_info response: {:?}", result);
        result
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        println!("[MCP DEBUG] list_tools called");
        let tools = self.tool_router.list_all();
        println!("[MCP DEBUG] list_tools response: {} tools available", tools.len());
        for tool in &tools {
            println!("[MCP DEBUG]   - {}: {:?}", tool.name, tool.description);
        }
        std::future::ready(Ok(ListToolsResult::with_all_items(tools)))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        println!("[MCP DEBUG] call_tool RECEIVED:");
        println!("[MCP DEBUG]   tool_name: {}", request.name);
        println!("[MCP DEBUG]   arguments: {:?}", request.arguments);
        let tool_ctx = ToolCallContext::new(self, request, context);
        async move {
            let result = self.tool_router.call(tool_ctx).await.map_err(Into::into);
            match &result {
                Ok(r) => {
                    println!("[MCP DEBUG] call_tool RESPONSE: success={}", !r.is_error.unwrap_or(false));
                    for content in &r.content {
                        println!("[MCP DEBUG]   content: {:?}", content);
                    }
                }
                Err(e) => {
                    println!("[MCP DEBUG] call_tool ERROR: {:?}", e);
                }
            }
            result
        }
    }

    fn on_initialized(&self, _context: rmcp::service::NotificationContext<rmcp::RoleServer>) -> impl std::future::Future<Output = ()> + Send + '_ {
        println!("[MCP DEBUG] on_initialized - MCP client connected and initialized");
        log::info!("MCP client initialized successfully");
        std::future::ready(())
    }

    fn on_cancelled(
        &self,
        notification: rmcp::model::CancelledNotificationParam,
        _context: rmcp::service::NotificationContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = ()> + Send + '_ {
        println!("[MCP DEBUG] on_cancelled - Request cancelled: {:?}", notification);
        log::info!("Request cancelled: {:?}", notification);
        std::future::ready(())
    }
}
