use crate::mcp::types::{CaptureScreenRequest, ConnectionRequest, RunScriptRequest, SendKeyRequest, SendTextRequest, TerminalState};
use crate::mcp::{McpCommand, ScriptResult, SenderType};
use crate::Address;

use parking_lot::Mutex;
use rmcp::{
    handler::server::{
        tool::{ToolCallContext, ToolRouter},
        wrapper::Parameters,
    },
    model::{
        Annotated, CallToolRequestParam, CallToolResult, Content, Implementation, InitializeResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParam, ProtocolVersion, RawResource, ReadResourceRequestParam, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
    },
    tool, tool_router, ErrorData as McpError, ServerHandler,
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

/// Embedded scripting documentation for MCP resources
const SCRIPTING_DOC: &str = include_str!("../../SCRIPTING.md");

#[derive(Clone)]
pub struct IcyTermMcpHandler {
    command_tx: mpsc::UnboundedSender<McpCommand>,
    pub tool_router: ToolRouter<Self>,
    run_script_singleflight: Arc<tokio::sync::Mutex<()>>,
}

#[tool_router]
impl IcyTermMcpHandler {
    pub fn new(command_tx: mpsc::UnboundedSender<McpCommand>) -> Self {
        Self {
            command_tx,
            tool_router: Self::tool_router(),
            run_script_singleflight: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    #[tool(
        description = "Connect to a BBS. After username you need to send an enter. As well after the password to log in. Both need to be entered briefly behind each other."
    )]
    async fn connect(&self, params: Parameters<ConnectionRequest>) -> Result<CallToolResult, McpError> {
        self.command_tx
            .send(McpCommand::Connect(params.0.url.clone()))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!("Connecting to {}", params.0.url))]))
    }

    #[tool(description = "Disconnect from the current BBS")]
    async fn disconnect(&self) -> Result<CallToolResult, McpError> {
        self.command_tx
            .send(McpCommand::Disconnect)
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text("Disconnected")]))
    }

    #[tool(description = "Send text to the terminal (supports \\n for Enter, \\t for Tab)")]
    async fn send_text(&self, params: Parameters<SendTextRequest>) -> Result<CallToolResult, McpError> {
        let text = params.0.text;

        let processed_text = text.replace("\\n", "\r").replace("\\r", "\r").replace("\\t", "\t").replace("\\e", "\x1b");

        self.command_tx
            .send(McpCommand::SendText(processed_text))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text("Sent")]))
    }

    #[tool(description = "Send a special key (Enter, Tab, Escape, etc.)")]
    async fn send_key(&self, params: Parameters<SendKeyRequest>) -> Result<CallToolResult, McpError> {
        self.command_tx
            .send(McpCommand::SendKey(params.0.key.clone()))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!("Sent key: {}", params.0.key))]))
    }

    #[tool(description = "Capture the current terminal screen")]
    async fn capture_screen(&self, params: Parameters<CaptureScreenRequest>) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::CaptureScreen(params.0.format, Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let data = response_rx.await.map_err(|_| McpError::internal_error("Failed to capture screen", None))?;

        let text = String::from_utf8_lossy(&data).to_string();
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(description = "Get current terminal state")]
    async fn get_state(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::GetState(Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let state: TerminalState = response_rx.await.map_err(|_| McpError::internal_error("Failed to get state", None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Terminal State:\nCursor: {:?}\nScreen: {:?}\nConnected: {}\nCurrent BBS: {:?}",
            state.cursor_position, state.screen_size, state.is_connected, state.current_bbs
        ))]))
    }

    #[tool(
        description = "List available BBS addresses from the address book (includes username/password). Treat the returned credentials as secret; do not display them to end users."
    )]
    async fn list_addresses(&self) -> Result<CallToolResult, McpError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(McpCommand::ListAddresses(Arc::new(Mutex::new(Some(response_tx)))))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let addresses: Vec<Address> = response_rx.await.map_err(|_| McpError::internal_error("Failed to list addresses", None))?;

        let text = addresses
            .iter()
            .map(|a| {
                let mut entry = format!("• {} - {}", a.system_name, a.address);

                if !a.user_name.is_empty() {
                    entry.push_str(&format!("\n  Username: {}", a.user_name));
                }

                if !a.password.is_empty() {
                    entry.push_str(&format!("\n  Password: {}", a.password));
                }

                entry.push_str(&format!("\n  Protocol: {:?}", a.protocol));
                entry.push_str(&format!("\n  Terminal: {:?}", a.terminal_type));
                entry
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let out = if text.is_empty() {
            "No addresses found".to_string()
        } else {
            format!("BBS Directory:\n\n{}", text)
        };

        Ok(CallToolResult::success(vec![Content::text(out)]))
    }

    #[tool(description = "Clear the terminal screen")]
    async fn clear_screen(&self) -> Result<CallToolResult, McpError> {
        self.command_tx
            .send(McpCommand::ClearScreen)
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text("Screen cleared")]))
    }

    #[tool(description = "Run a Lua script for terminal automation.")]
    async fn run_script(&self, params: Parameters<RunScriptRequest>) -> Result<CallToolResult, McpError> {
        let guard = self
            .run_script_singleflight
            .try_lock()
            .map_err(|_| McpError::internal_error("run_script already running", None))?;

        let (response_tx, response_rx) = oneshot::channel();
        let sender: SenderType<ScriptResult> = Arc::new(Mutex::new(Some(response_tx)));

        self.command_tx
            .send(McpCommand::RunScript(params.0.script.clone(), Some(sender)))
            .map_err(|e| McpError::internal_error(format!("Failed to send command: {e}"), None))?;

        let result = tokio::time::timeout(Duration::from_secs(300), response_rx)
            .await
            .map_err(|_| McpError::internal_error("Script execution timed out (5 minutes)", None))
            .and_then(|r| r.map_err(|_| McpError::internal_error("Script execution channel closed unexpectedly", None)))?;

        drop(guard);

        match result {
            Ok(output) => {
                let text = if output.is_empty() {
                    "Script executed successfully".to_string()
                } else {
                    format!("Script output:\n{}", output)
                };
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(error) => Ok(CallToolResult::error(vec![Content::text(format!("Script error: {}", error))])),
        }
    }

    #[tool(
        description = "Get the complete Lua scripting API documentation for IcyTerm terminal automation. Call this before writing scripts to learn the available functions."
    )]
    async fn get_scripting_api(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(SCRIPTING_DOC)]))
    }
}

impl ServerHandler for IcyTermMcpHandler {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().enable_resources().build(),
            server_info: Implementation {
                name: "icy_term_mcp".to_string(),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some("IcyTerm MCP server (HTTP)".to_string()),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let tools = self.tool_router.list_all();
        std::future::ready(Ok(ListToolsResult::with_all_items(tools)))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let tool_ctx = ToolCallContext::new(self, request, context);
        async move { self.tool_router.call(tool_ctx).await.map_err(Into::into) }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        let mut raw = RawResource::new("icy_term://scripting_api", "IcyTerm Scripting API");
        raw.description = Some("Lua scripting API documentation for terminal automation".to_string());
        raw.mime_type = Some("text/markdown".to_string());
        let resource: Resource = Annotated::new(raw, None);
        std::future::ready(Ok(ListResourcesResult::with_all_items(vec![resource])))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        if request.uri != "icy_term://scripting_api" {
            return std::future::ready(Err(McpError::resource_not_found("Unknown resource", None)));
        }

        let contents = ResourceContents::TextResourceContents {
            uri: request.uri,
            mime_type: Some("text/markdown".to_string()),
            text: SCRIPTING_DOC.to_string(),
            meta: None,
        };

        std::future::ready(Ok(ReadResourceResult { contents: vec![contents] }))
    }

    fn on_initialized(&self, _context: rmcp::service::NotificationContext<rmcp::RoleServer>) -> impl std::future::Future<Output = ()> + Send + '_ {
        log::info!("MCP client initialized successfully");
        std::future::ready(())
    }

    fn on_cancelled(
        &self,
        notification: rmcp::model::CancelledNotificationParam,
        _context: rmcp::service::NotificationContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = ()> + Send + '_ {
        log::info!("Request cancelled: {:?}", notification);
        std::future::ready(())
    }
}
