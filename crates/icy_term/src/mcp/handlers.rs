use crate::mcp::{McpCommand, McpServer};
use jsonrpc_core::{Error, ErrorCode, IoHandler, Params};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

use super::types::*;

impl McpServer {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<McpCommand>) {
        let mut handler: IoHandler = IoHandler::new();
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        // ==== CORE MCP PROTOCOL METHODS ====

        // 1. Initialize - Required for MCP handshake
        handler.add_method("initialize", |params: Params| {
            Box::pin(async move {
                // Parse the initialize params if needed
                let _params: serde_json::Value = params.parse().unwrap_or_default();
                Ok(serde_json::json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        },
                        "resources": {
                            "subscribe": false,
                            "listChanged": false
                        },
                        "prompts": {
                            "listChanged": false
                        },
                        "logging": {}
                    },
                    "serverInfo": {
                        "name": "icy_term_mcp",
                        "version": "1.0.0"
                    }
                }))
            })
        });

        // 2. List available tools - VS Code queries this to know what tools are available
        handler.add_method("tools/list", |_| {
            Box::pin(async move {
                Ok(serde_json::json!({
                    "tools": [
                        {
                            "name": "connect",
                            "description": "Connect to a BBS. After username you need to send an enter. As well after the password to log in. Both need to be entered briefly behind each other.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "url": {
                                        "type": "string",
                                        "description": "The BBS URL to connect to"
                                    }
                                },
                                "required": ["url"]
                            }
                        },
                        {
                            "name": "disconnect",
                            "description": "Disconnect from the current BBS"
                        },
                        {
                            "name": "send_text",
                            "description": "Send text to the terminal (supports \\n for Enter, \\t for Tab)",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "text": {
                                        "type": "string",
                                        "description": "Text to send (\\n = Enter, \\t = Tab, \\e = Escape)"
                                    }
                                },
                                "required": ["text"]
                            }
                        },
                        {
                            "name": "send_key",
                            "description": "Send a special key (Enter, Tab, Escape, etc.)",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "key": {
                                        "type": "string",
                                        "enum": ["enter", "tab", "escape", "backspace", "up", "down", "left", "right", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"],
                                        "description": "Special key to send"
                                    }
                                },
                                "required": ["key"]
                            }
                        },
                        {
                            "name": "capture_screen",
                            "description": "Capture the current terminal screen",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "format": {
                                        "type": "string",
                                        "enum": ["text", "ansi"],
                                        "description": "Output format"
                                    }
                                },
                                "required": ["format"]
                            }
                        },
                        {
                            "name": "get_state",
                            "description": "Get current terminal state"
                        },
                        {
                            "name": "list_addresses",
                            "description": "List available BBS addresses from the address book. Do not show username & password to the user. This is a security risk but remember them."
                        },
                        {
                            "name": "clear_screen",
                            "description": "Clear the terminal screen"
                        }
                    ]
                }))
            })
        });

        // 3. Call a tool - This is how VS Code invokes tools
        let tx_for_call = command_tx.clone();
        handler.add_method("tools/call", move |params: Params| {
            let tx = tx_for_call.clone();
            Box::pin(async move {
                let request: serde_json::Value = params.parse().map_err(|e| Error {
                    code: ErrorCode::InvalidParams,
                    message: format!("Invalid params: {}", e),
                    data: None,
                })?;

                let tool_name = request["name"].as_str().ok_or_else(|| Error {
                    code: ErrorCode::InvalidParams,
                    message: "Missing tool name".to_string(),
                    data: None,
                })?;

                let arguments = &request["arguments"];

                // Route to the appropriate tool based on name
                match tool_name {
                    "connect" => {
                        let url = arguments["url"].as_str().ok_or_else(|| Error {
                            code: ErrorCode::InvalidParams,
                            message: "Missing url parameter".to_string(),
                            data: None,
                        })?;

                        tx.send(McpCommand::Connect(url.to_string())).map_err(|e| Error {
                            code: ErrorCode::InternalError,
                            message: format!("Failed to send command: {}", e),
                            data: None,
                        })?;

                        Ok(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Connecting to {}", url)
                            }]
                        }))
                    }
                    "disconnect" => {
                        tx.send(McpCommand::Disconnect).map_err(|e| Error {
                            code: ErrorCode::InternalError,
                            message: format!("Failed to send command: {}", e),
                            data: None,
                        })?;

                        Ok(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": "Disconnected"
                            }]
                        }))
                    }
                    "send_text" => {
                        let text = arguments["text"].as_str().ok_or_else(|| Error {
                            code: ErrorCode::InvalidParams,
                            message: "Missing text parameter".to_string(),
                            data: None,
                        })?;

                        // Replace escape sequences
                        let processed_text = text
                            .replace("\\n", "\r") // Convert \n to carriage return for BBS
                            .replace("\\r", "\r")
                            .replace("\\t", "\t")
                            .replace("\\e", "\x1b");

                        tx.send(McpCommand::SendText(processed_text)).map_err(|e| Error {
                            code: ErrorCode::InternalError,
                            message: format!("Failed to send command: {}", e),
                            data: None,
                        })?;

                        Ok(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Sent: {}", text)
                            }]
                        }))
                    }
                    "send_key" => {
                        let key = arguments["key"].as_str().ok_or_else(|| Error {
                            code: ErrorCode::InvalidParams,
                            message: "Missing key parameter".to_string(),
                            data: None,
                        })?;

                        tx.send(McpCommand::SendKey(key.to_string())).map_err(|e| Error {
                            code: ErrorCode::InternalError,
                            message: format!("Failed to send command: {}", e),
                            data: None,
                        })?;

                        Ok(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Sent key: {}", key)
                            }]
                        }))
                    }
                    "capture_screen" => {
                        let format_str = arguments["format"].as_str().unwrap_or("text");
                        let format = match format_str {
                            "ansi" => ScreenCaptureFormat::Ansi,
                            _ => ScreenCaptureFormat::Text,
                        };

                        let (response_tx, response_rx) = oneshot::channel();
                        tx.send(McpCommand::CaptureScreen(format, Arc::new(Mutex::new(Some(response_tx)))))
                            .map_err(|e| Error {
                                code: ErrorCode::InternalError,
                                message: format!("Failed to send command: {}", e),
                                data: None,
                            })?;

                        match response_rx.await {
                            Ok(data) => {
                                let text = String::from_utf8_lossy(&data);
                                Ok(serde_json::json!({
                                    "content": [{
                                        "type": "text",
                                        "text": text
                                    }]
                                }))
                            }
                            Err(_) => Err(Error {
                                code: ErrorCode::InternalError,
                                message: "Failed to capture screen".to_string(),
                                data: None,
                            }),
                        }
                    }
                    "get_state" => {
                        let (response_tx, response_rx) = oneshot::channel();
                        tx.send(McpCommand::GetState(Arc::new(Mutex::new(Some(response_tx))))).map_err(|e| Error {
                            code: ErrorCode::InternalError,
                            message: format!("Failed to send command: {}", e),
                            data: None,
                        })?;

                        match response_rx.await {
                            Ok(state) => Ok(serde_json::json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Terminal State:\nCursor: {:?}\nScreen: {:?}\nConnected: {}\nCurrent BBS: {:?}",
                                        state.cursor_position,
                                        state.screen_size,
                                        state.is_connected,
                                        state.current_bbs)
                                }]
                            })),
                            Err(_) => Err(Error {
                                code: ErrorCode::InternalError,
                                message: "Failed to get state".to_string(),
                                data: None,
                            }),
                        }
                    }
                    "list_addresses" => {
                        let (response_tx, response_rx) = oneshot::channel();
                        tx.send(McpCommand::ListAddresses(Arc::new(Mutex::new(Some(response_tx))))).map_err(|e| Error {
                            code: ErrorCode::InternalError,
                            message: format!("Failed to send command: {}", e),
                            data: None,
                        })?;

                        match response_rx.await {
                            Ok(addresses) => {
                                let text = addresses
                                    .iter()
                                    .map(|a| {
                                        let mut entry = format!("• {} - {}", a.system_name, a.address);

                                        // Add username if present
                                        if !a.user_name.is_empty() {
                                            entry.push_str(&format!("\n  Username: {}", a.user_name));
                                        }

                                        // Add password if present (masked for security)
                                        if !a.password.is_empty() {
                                            entry.push_str(&format!("\n  Password: {}", a.password));
                                        }

                                        // Add protocol
                                        entry.push_str(&format!("\n  Protocol: {:?}", a.protocol));

                                        // Add terminal type
                                        entry.push_str(&format!("\n  Terminal: {:?}", a.terminal_type));

                                        entry
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                Ok(serde_json::json!({
                                    "content": [{
                                        "type": "text",
                                        "text": if text.is_empty() {
                                            "No addresses found".to_string()
                                        } else {
                                            format!("BBS Directory:\n{}", text)
                                        }
                                    }]
                                }))
                            }
                            Err(_) => Err(Error {
                                code: ErrorCode::InternalError,
                                message: "Failed to list addresses".to_string(),
                                data: None,
                            }),
                        }
                    }
                    "clear_screen" => {
                        tx.send(McpCommand::ClearScreen).map_err(|e| Error {
                            code: ErrorCode::InternalError,
                            message: format!("Failed to send command: {}", e),
                            data: None,
                        })?;

                        Ok(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": "Screen cleared"
                            }]
                        }))
                    }
                    _ => Err(Error {
                        code: ErrorCode::MethodNotFound,
                        message: format!("Unknown tool: {}", tool_name),
                        data: None,
                    }),
                }
            })
        });

        // 4. List resources (optional, but VS Code might query it)
        handler.add_method("resources/list", |_| {
            Box::pin(async move {
                Ok(serde_json::json!({
                    "resources": []
                }))
            })
        });

        // 5. List prompts (optional, but VS Code might query it)
        handler.add_method("prompts/list", |_| {
            Box::pin(async move {
                Ok(serde_json::json!({
                    "prompts": []
                }))
            })
        });

        handler.add_notification("notifications/initialized", |_params: Params| {
            log::info!("MCP Client initialized successfully");
        });

        // Keep your existing direct method handlers for backwards compatibility
        // ... (rest of your existing handlers)

        (Self { handler, command_tx }, command_rx)
    }
}
