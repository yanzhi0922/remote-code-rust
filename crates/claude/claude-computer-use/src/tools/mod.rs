pub mod keyboard;
pub mod mouse;
pub mod screenshot;
pub mod scroll;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub fn all_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "screenshot",
            description: "Capture a screenshot of the desktop. Returns image metadata with file path.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "monitor": {"type": "integer", "description": "Monitor index (0=primary)", "default": 0},
                    "format": {"type": "string", "enum": ["png", "jpeg"], "default": "png"},
                    "quality": {"type": "integer", "description": "JPEG quality 1-100"}
                }
            }),
        },
        ToolSpec {
            name: "get_screen_size",
            description: "Get screen dimensions and monitor count.",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolSpec {
            name: "mouse_move",
            description: "Move the mouse cursor to absolute (x, y) coordinates.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer", "description": "X coordinate in pixels"},
                    "y": {"type": "integer", "description": "Y coordinate in pixels"}
                },
                "required": ["x", "y"]
            }),
        },
        ToolSpec {
            name: "mouse_click",
            description: "Click the mouse at the specified coordinates.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer", "description": "X coordinate"},
                    "y": {"type": "integer", "description": "Y coordinate"},
                    "button": {"type": "string", "enum": ["left", "right", "middle"], "default": "left"},
                    "double": {"type": "boolean", "default": false}
                },
                "required": ["x", "y"]
            }),
        },
        ToolSpec {
            name: "mouse_drag",
            description: "Click and drag from one point to another.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from_x": {"type": "integer"},
                    "from_y": {"type": "integer"},
                    "to_x": {"type": "integer"},
                    "to_y": {"type": "integer"},
                    "button": {"type": "string", "enum": ["left", "right"], "default": "left"}
                },
                "required": ["from_x", "from_y", "to_x", "to_y"]
            }),
        },
        ToolSpec {
            name: "type_text",
            description: "Type a text string using the keyboard.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to type"}
                },
                "required": ["text"]
            }),
        },
        ToolSpec {
            name: "key_press",
            description: "Press a key or key combination (e.g. \"enter\", \"ctrl+c\", \"alt+f4\").",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Key or combo: \"enter\", \"ctrl+c\", \"alt+f4\", \"tab\", \"escape\""},
                    "count": {"type": "integer", "default": 1}
                },
                "required": ["key"]
            }),
        },
        ToolSpec {
            name: "scroll",
            description: "Scroll the mouse wheel at the given coordinates.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "direction": {"type": "string", "enum": ["up", "down"], "default": "down"},
                    "amount": {"type": "integer", "description": "Number of scroll steps", "default": 3}
                },
                "required": ["x", "y"]
            }),
        },
    ]
}

pub async fn dispatch_tool(name: &str, input: &Value) -> Result<String> {
    match name {
        "screenshot" => screenshot::screenshot(input).await,
        "get_screen_size" => screenshot::get_screen_size(input).await,
        "mouse_move" => mouse::mouse_move(input).await,
        "mouse_click" => mouse::mouse_click(input).await,
        "mouse_drag" => mouse::mouse_drag(input).await,
        "type_text" => keyboard::type_text(input).await,
        "key_press" => keyboard::key_press(input).await,
        "scroll" => scroll::scroll(input).await,
        _ => Err(anyhow!("unknown tool: {name}")),
    }
}
