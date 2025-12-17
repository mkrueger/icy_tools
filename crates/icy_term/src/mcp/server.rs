use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::mcp::McpCommand;
use crate::mcp::handlers::IcyTermMcpHandler;

pub struct McpServer {
    pub handler: IcyTermMcpHandler,
}

impl McpServer {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<McpCommand>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let handler = IcyTermMcpHandler::new(command_tx);
        (Self { handler }, command_rx)
    }

    pub async fn start(self: Arc<Self>, port: u16) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let session_manager = Arc::new(LocalSessionManager::default());
        let config = StreamableHttpServerConfig {
            // We want a robust, spec-aligned HTTP MCP server with sessions.
            stateful_mode: true,
            ..Default::default()
        };

        // rmcp's StreamableHttpService expects a factory that can create a fresh service per session.
        let handler = self.handler.clone();
        let mcp_service = StreamableHttpService::new(move || Ok(handler.clone()), session_manager, config);

        let app = Router::new().route_service("/", mcp_service);

        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        log::info!("MCP Streamable HTTP server listening on http://127.0.0.1:{}", port);

        axum::serve(listener, app).await?;
        Ok(())
    }
}
