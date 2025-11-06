use crate::mcp::McpCommand;
use jsonrpc_core::IoHandler;
use std::sync::Arc;
use tokio::sync::mpsc;
use warp::{Filter, Reply};

pub struct McpServer {
    pub handler: IoHandler,
    pub command_tx: mpsc::UnboundedSender<McpCommand>,
}

impl McpServer {
    pub async fn start(self: Arc<Self>, port: u16) -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Clone for the closure
        let server = self.clone();

        // Create CORS configuration
        let cors = warp::cors()
            .allow_any_origin()
            .allow_methods(vec!["POST", "OPTIONS"])
            .allow_headers(vec!["Content-Type"]);

        // Create the POST route for JSON-RPC
        let json_rpc_route = warp::post()
            .and(warp::path::end())
            .and(warp::body::bytes())
            .and(warp::any().map(move || server.clone()))
            .and_then(handle_request)
            .with(cors);

        // Start the server
        log::info!("MCP HTTP server listening on http://127.0.0.1:{}", port);
        warp::serve(json_rpc_route).run(([127, 0, 0, 1], port)).await;

        Ok(())
    }
}

async fn handle_request(body: bytes::Bytes, server: Arc<McpServer>) -> Result<impl Reply, warp::Rejection> {
    let json_request = String::from_utf8_lossy(&body);
    log::debug!("MCP Request: {}", json_request);
    // Handle the JSON-RPC request
    let response = server.handler.handle_request(&json_request).await;
    log::debug!("MCP Response: {:?}", response);
    // Return the response
    let response_body = response.unwrap_or_else(|| {
        serde_json::json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32603,
                "message": "Internal error"
            },
            "id": null
        })
        .to_string()
    });

    Ok(warp::reply::with_header(response_body, "content-type", "application/json"))
}
