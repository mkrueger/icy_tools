use crate::mcp::{McpCommand, McpServer};
use jsonrpc_core::{Error, ErrorCode, IoHandler, Params};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use super::types::*;

/// Embedded scripting documentation for MCP resources
const SCRIPTING_DOC: &str = include_str!("../../SCRIPTING.md");

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
                            "description": "Disconnect from the current BBS",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
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
                            "description": "Get current terminal state",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "list_addresses",
                            "description": "List available BBS addresses from the address book. Do not show username & password to the user. This is a security risk but remember them.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "clear_screen",
                            "description": "Clear the terminal screen",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "run_script",
                            "description": "Run a Lua script for terminal automation.\n\nIMPORTANT API Functions (use ONLY these):\n- connect(url) - Connect to BBS\n- disconnect() - Disconnect\n- send(text) - Send text (use \\r for Enter)\n- send_key(key) - Send special key: enter, tab, escape, up, down, left, right, f1-f12\n- send_login() - Send stored username + password from address book\n- send_username() / send_password() - Send credentials separately\n- wait_for(pattern, timeout_ms) - Wait for text on screen (regex supported, default 30000ms)\n- on_screen(pattern) - Check if text is visible (returns true/false, no wait)\n- sleep(ms) - Wait milliseconds\n- println(text) - Print debug message\n- is_connected() - Check connection status\n\nExample login script:\n```lua\nwait_for('login:', 10000)\nsend_username()\nsend('\\r')\nwait_for('password:', 5000)\nsend_password()\nsend('\\r')\n```",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "script": {
                                        "type": "string",
                                        "description": "The Lua script code to execute"
                                    }
                                },
                                "required": ["script"]
                            }
                        },
                        {
                            "name": "get_scripting_api",
                            "description": "Get the complete Lua scripting API documentation for IcyTerm terminal automation. Call this before writing scripts to learn the available functions.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
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
                        // No arguments needed, just send the command
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
                        // Check if text exists, provide empty string as default
                        let text = arguments["text"].as_str().unwrap_or("");

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
                        // Check if key exists, return error if missing since it's required
                        let key = arguments["key"].as_str();
                        if key.is_none() {
                            return Err(Error {
                                code: ErrorCode::InvalidParams,
                                message: "Missing required 'key' parameter".to_string(),
                                data: None,
                            });
                        }
                        let key = key.unwrap();

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
                        // Default to "text" if format is missing
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
                        // No arguments needed
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

                    "clear_screen" => {
                        // No arguments needed
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
                    "list_addresses" => {
                        // No arguments needed for list_addresses, so we just proceed
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

                                        // Add password if present (show actual password as requested)
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
                                    .join("\n\n");

                                Ok(serde_json::json!({
                                    "content": [{
                                        "type": "text",
                                        "text": if text.is_empty() {
                                            "No addresses found".to_string()
                                        } else {
                                            format!("BBS Directory:\n\n{}", text)
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

                    "run_script" => {
                        let script = arguments["script"].as_str();
                        if script.is_none() {
                            return Err(Error {
                                code: ErrorCode::InvalidParams,
                                message: "Missing required 'script' parameter".to_string(),
                                data: None,
                            });
                        }
                        let script = script.unwrap();

                        let (response_tx, response_rx) = oneshot::channel();
                        tx.send(McpCommand::RunScript(script.to_string(), Some(Arc::new(Mutex::new(Some(response_tx))))))
                            .map_err(|e| Error {
                                code: ErrorCode::InternalError,
                                message: format!("Failed to send command: {}", e),
                                data: None,
                            })?;

                        // Wait for script to complete (with 5 minute timeout)
                        match tokio::time::timeout(std::time::Duration::from_secs(300), response_rx).await {
                            Ok(Ok(result)) => match result {
                                Ok(output) => Ok(serde_json::json!({
                                    "content": [{
                                        "type": "text",
                                        "text": if output.is_empty() {
                                            "Script executed successfully".to_string()
                                        } else {
                                            format!("Script output:\n{}", output)
                                        }
                                    }]
                                })),
                                Err(error) => Ok(serde_json::json!({
                                    "content": [{
                                        "type": "text",
                                        "text": format!("Script error: {}", error)
                                    }],
                                    "isError": true
                                })),
                            },
                            Ok(Err(_)) => Err(Error {
                                code: ErrorCode::InternalError,
                                message: "Script execution channel closed unexpectedly".to_string(),
                                data: None,
                            }),
                            Err(_) => Err(Error {
                                code: ErrorCode::InternalError,
                                message: "Script execution timed out (5 minutes)".to_string(),
                                data: None,
                            }),
                        }
                    }

                    "get_scripting_api" => {
                        Ok(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": SCRIPTING_DOC
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

        // 4. List resources - expose scripting documentation
        handler.add_method("resources/list", |_| {
            Box::pin(async move {
                Ok(serde_json::json!({
                    "resources": [
                        {
                            "uri": "icy_term://scripting_api",
                            "name": "IcyTerm Scripting API",
                            "description": "Lua scripting API documentation for terminal automation",
                            "mimeType": "text/markdown"
                        }
                    ]
                }))
            })
        });

        // 4b. Read resources - return scripting documentation content
        handler.add_method("resources/read", |params: Params| {
            Box::pin(async move {
                let request: serde_json::Value = params.parse().map_err(|e| Error {
                    code: ErrorCode::InvalidParams,
                    message: format!("Invalid params: {}", e),
                    data: None,
                })?;

                let uri = request["uri"].as_str().ok_or_else(|| Error {
                    code: ErrorCode::InvalidParams,
                    message: "Missing 'uri' parameter".to_string(),
                    data: None,
                })?;

                match uri {
                    "icy_term://scripting_api" => Ok(serde_json::json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "text/markdown",
                            "text": SCRIPTING_DOC
                        }]
                    })),
                    _ => Err(Error {
                        code: ErrorCode::InvalidParams,
                        message: format!("Unknown resource: {}", uri),
                        data: None,
                    }),
                }
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

        handler.add_notification("notifications/cancelled", |params: Params| {
            if let Ok(value) = params.parse::<serde_json::Value>() {
                let request_id = value.get("requestId").and_then(|v| v.as_i64());
                let reason = value.get("reason").and_then(|v| v.as_str());
                log::info!("Request {:?} cancelled: {:?}", request_id, reason);
            }
        });

        (Self { handler, command_tx }, command_rx)
    }
}
