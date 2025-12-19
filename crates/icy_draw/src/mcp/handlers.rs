//! MCP tool handlers for icy_draw

use crate::mcp::McpCommand;
use crate::mcp::types::{
    AnimationGetScreenRequest, AnimationGetTextRequest, AnimationReplaceTextRequest, BitFontGetCharRequest, BitFontSetCharRequest, CharListResponse,
    EditorStatus, GetHelpRequest, LoadDocumentRequest, NewDocumentRequest,
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

    #[tool(description = "Get documentation. Without 'editor' parameter: general overview. With 'animation' or 'bitfont': editor-specific documentation.")]
    async fn get_help(&self, params: Parameters<GetHelpRequest>) -> Result<CallToolResult, McpError> {
        let doc = match params.0.editor.as_deref() {
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
