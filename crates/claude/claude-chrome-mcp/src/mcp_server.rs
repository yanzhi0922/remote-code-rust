//! MCP server for Chrome browser automation.
//!
//! Implements the MCP (Model Context Protocol) server interface, exposing
//! browser automation tools via stdio transport.

use anyhow::Result;
use serde_json::{Value, json};
use std::io::{BufRead, Write};

use crate::tools;

/// Run the Chrome MCP server, communicating over stdin/stdout.
pub async fn run_stdio_server() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    // Send initialization response.
    let init_response = json!({
        "jsonrpc": "2.0",
        "id": null,
        "result": {
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "chrome-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    });
    writeln!(stdout, "{}", init_response)?;
    stdout.flush()?;

    let mut line = String::new();
    let mut stdin_lock = stdin.lock();

    while stdin_lock.read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("invalid JSON from client: {e}");
                line.clear();
                continue;
            }
        };

        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or("");
        let params = request["params"].clone();

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "chrome-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            "tools/list" => {
                let tool_list: Vec<Value> = tools::all_tool_specs()
                    .iter()
                    .map(|spec| {
                        json!({
                            "name": spec.name,
                            "description": spec.description,
                            "inputSchema": spec.input_schema,
                        })
                    })
                    .collect();
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": tool_list
                    }
                })
            }
            "tools/call" => {
                let tool_name = params["name"].as_str().unwrap_or("");
                let tool_input = params.get("arguments").cloned().unwrap_or(json!({}));

                match tools::dispatch_tool(tool_name, &tool_input).await {
                    Ok(result_text) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": result_text
                            }]
                        }
                    }),
                    Err(e) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "isError": true,
                            "content": [{
                                "type": "text",
                                "text": format!("Tool error: {e}")
                            }]
                        }
                    }),
                }
            }
            "notifications/initialized" => {
                line.clear();
                continue;
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("method not found: {method}")
                }
            }),
        };

        writeln!(stdout, "{}", response)?;
        stdout.flush()?;
        line.clear();
    }

    Ok(())
}
