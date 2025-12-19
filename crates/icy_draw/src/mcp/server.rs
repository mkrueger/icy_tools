//! MCP HTTP server for icy_draw

use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::mcp::McpCommand;
use crate::mcp::handlers::IcyDrawMcpHandler;

/// MCP server instance
pub struct McpServer {
    pub handler: IcyDrawMcpHandler,
}

impl McpServer {
    /// Create a new MCP server with a command channel
    pub fn new() -> (Self, mpsc::UnboundedReceiver<McpCommand>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let handler = IcyDrawMcpHandler::new(command_tx);
        (Self { handler }, command_rx)
    }

    /// Start the HTTP server on the specified port
    pub async fn start(self: Arc<Self>, port: u16) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let session_manager = Arc::new(LocalSessionManager::default());
        let config = StreamableHttpServerConfig {
            stateful_mode: true,
            ..Default::default()
        };

        let handler = self.handler.clone();
        let mcp_service = StreamableHttpService::new(move || Ok(handler.clone()), session_manager, config);

        let app = Router::new().route_service("/", mcp_service);

        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        println!("[MCP DEBUG] ========================================");
        println!("[MCP DEBUG] MCP Server starting on http://127.0.0.1:{}", port);
        println!("[MCP DEBUG] Waiting for connections...");
        println!("[MCP DEBUG] ========================================");
        log::info!("MCP Streamable HTTP server listening on http://127.0.0.1:{}", port);

        axum::serve(listener, app).await?;
        Ok(())
    }
}
