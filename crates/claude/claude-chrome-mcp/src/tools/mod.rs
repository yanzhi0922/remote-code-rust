//! Tool dispatch: maps tool names to handler functions.

pub mod click;
pub mod console;
pub mod evaluate_js;
pub mod find;
pub mod form_input;
pub mod get_page_text;
pub mod navigate;
pub mod network;
pub mod read_page;
pub mod resize;
pub mod screenshot;
pub mod tabs;
pub mod type_text;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

/// Dispatch a tool call by name.
pub async fn dispatch_tool(name: &str, input: &Value) -> Result<String> {
    match name {
        "navigate" => navigate::navigate(input).await,
        "read_page" => read_page::read_page(input).await,
        "get_page_text" => get_page_text::get_page_text(input).await,
        "find" => find::find(input).await,
        "click" => click::click(input).await,
        "type_text" => type_text::type_text(input).await,
        "evaluate_js" => evaluate_js::evaluate_js(input).await,
        "screenshot" => screenshot::screenshot(input).await,
        "tabs_list" => tabs::tabs_list(input).await,
        "tabs_create" => tabs::tabs_create(input).await,
        "tabs_close" => tabs::tabs_close(input).await,
        "console_messages" => console::console_messages(input).await,
        "network_requests" => network::network_requests(input).await,
        "form_input" => form_input::form_input(input).await,
        "resize_window" => resize::resize_window(input).await,
        _ => Err(anyhow!("unknown tool: {name}")),
    }
}

/// Tool specification: name, description, and JSON Schema.
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// All registered tools with their schemas.
pub fn all_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "navigate",
            description: "Navigate a browser tab to a URL.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to navigate to"},
                    "waitUntil": {"type": "string", "enum": ["load", "domcontentloaded", "none"], "default": "load"}
                },
                "required": ["url"]
            }),
        },
        ToolSpec {
            name: "read_page",
            description: "Capture high-level page state from the active tab.",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolSpec {
            name: "get_page_text",
            description: "Read visible page text from the active tab.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "selector": {"type": "string", "description": "CSS selector (default: body)"},
                    "maxChars": {"type": "integer", "description": "Max chars to return", "default": 100000}
                }
            }),
        },
        ToolSpec {
            name: "find",
            description: "Find a pattern within page content.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex pattern to search"},
                    "selector": {"type": "string", "description": "CSS selector to scope search"}
                },
                "required": ["pattern"]
            }),
        },
        ToolSpec {
            name: "click",
            description: "Click an element by CSS selector or coordinates.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "selector": {"type": "string", "description": "CSS selector of element to click"},
                    "x": {"type": "number", "description": "X coordinate for coordinate-based click"},
                    "y": {"type": "number", "description": "Y coordinate for coordinate-based click"}
                }
            }),
        },
        ToolSpec {
            name: "type_text",
            description: "Type text into an element identified by CSS selector.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "selector": {"type": "string", "description": "CSS selector of input element"},
                    "text": {"type": "string", "description": "Text to type"},
                    "clear": {"type": "boolean", "description": "Clear existing text first", "default": true}
                },
                "required": ["selector", "text"]
            }),
        },
        ToolSpec {
            name: "evaluate_js",
            description: "Run page-scoped JavaScript in the browser tab.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "JavaScript expression to evaluate"},
                    "maxChars": {"type": "integer", "description": "Max chars for result", "default": 50000}
                },
                "required": ["expression"]
            }),
        },
        ToolSpec {
            name: "screenshot",
            description: "Capture a screenshot of the browser page.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "fullPage": {"type": "boolean", "description": "Capture full scrollable page", "default": false},
                    "selector": {"type": "string", "description": "CSS selector to clip to specific element"},
                    "format": {"type": "string", "enum": ["png", "jpeg"], "default": "png"},
                    "quality": {"type": "integer", "description": "JPEG quality 0-100"}
                }
            }),
        },
        ToolSpec {
            name: "tabs_list",
            description: "List or inspect browser tabs.",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolSpec {
            name: "tabs_create",
            description: "Create a new browser tab.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to open", "default": "about:blank"}
                }
            }),
        },
        ToolSpec {
            name: "tabs_close",
            description: "Close a browser tab.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL substring to match tab to close"}
                }
            }),
        },
        ToolSpec {
            name: "console_messages",
            description: "Read browser console messages.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex pattern to filter messages"},
                    "level": {"type": "string", "enum": ["all", "log", "warn", "error", "info"], "default": "all"},
                    "limit": {"type": "integer", "default": 100}
                }
            }),
        },
        ToolSpec {
            name: "network_requests",
            description: "Read captured network requests.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "urlPattern": {"type": "string", "description": "Regex pattern to filter URLs"},
                    "limit": {"type": "integer", "default": 100}
                }
            }),
        },
        ToolSpec {
            name: "form_input",
            description: "Fill or update form inputs in the page.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "selector": {"type": "string", "description": "CSS selector of the form element"},
                    "value": {"type": "string", "description": "Value to set"}
                },
                "required": ["selector", "value"]
            }),
        },
        ToolSpec {
            name: "resize_window",
            description: "Resize the browser window.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "width": {"type": "integer", "description": "Window width in pixels"},
                    "height": {"type": "integer", "description": "Window height in pixels"}
                },
                "required": ["width", "height"]
            }),
        },
    ]
}
