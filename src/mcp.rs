use crate::control::Action;
use crate::input;
use crate::keys;
use crate::session::Session;
use crate::wire::Result;
use base64::Engine;
use serde_json::{json, Map, Value};

const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const INSTRUCTIONS: &str = "Control one authorized remote computer over an UltraViewer-compatible session. Typical loop: connect -> session_status -> screenshot -> click/type_text/press_keys -> screenshot to check the result. Mouse coordinates are pixels in the most recent screenshot. The remote computer may allow viewing only. Only connect to computers you are authorized to access.";

#[derive(Default)]
pub struct Server {
    session: Session,
}

struct ToolResult {
    content: Vec<Value>,
    is_error: bool,
}

impl ToolResult {
    fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![json!({"type": "text", "text": text.into()})],
            is_error: false,
        }
    }

    fn error(error: impl Into<String>) -> Self {
        Self {
            content: vec![json!({"type": "text", "text": error.into()})],
            is_error: true,
        }
    }

    fn into_value(self) -> Value {
        json!({"content": self.content, "isError": self.is_error})
    }
}

impl Server {
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(_) => return Some(error_response(Value::Null, -32700, "Parse error")),
        };
        let id = message.get("id").cloned();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return id.map(|id| error_response(id, -32600, "Invalid request"));
        };
        let id = id?;
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let version = if PROTOCOL_VERSIONS.contains(&requested) {
                    requested
                } else {
                    PROTOCOL_VERSIONS[0]
                };
                Some(response(
                    id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": {"tools": {"listChanged": false}},
                        "serverInfo": {"name": "ultraviewer-mcp", "title": "UltraViewer MCP", "version": env!("CARGO_PKG_VERSION")},
                        "instructions": INSTRUCTIONS
                    }),
                ))
            }
            "ping" => Some(response(id, json!({}))),
            "tools/list" => Some(response(id, json!({"tools": tool_definitions()}))),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error_response(id, -32602, "Missing tool name"));
                };
                if !tool_definitions().iter().any(|tool| tool["name"] == name) {
                    return Some(error_response(id, -32602, &format!("Unknown tool: {name}")));
                }
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                if !arguments.is_object() {
                    return Some(error_response(
                        id,
                        -32602,
                        "Tool arguments must be an object",
                    ));
                }
                let result = match self.call(name, &arguments) {
                    Ok(result) => result,
                    Err(error) => ToolResult::error(error.to_string()),
                };
                Some(response(id, result.into_value()))
            }
            _ => Some(error_response(
                id,
                -32601,
                &format!("Method not found: {method}"),
            )),
        }
    }

    fn call(&mut self, name: &str, arguments: &Value) -> Result<ToolResult> {
        match name {
            "session_status" => Ok(ToolResult::text(self.session.status())),
            "connect" => {
                let partner_id = required_string(arguments, "partner_id")?
                    .chars()
                    .filter(|character| !character.is_whitespace() && *character != '-')
                    .collect::<String>();
                let password = required_string(arguments, "password")?;
                Ok(ToolResult::text(
                    self.session.connect(&partner_id, password)?,
                ))
            }
            "disconnect" => Ok(ToolResult::text(self.session.disconnect())),
            "screenshot" => {
                let max_width = optional_integer(arguments, "max_width")?.unwrap_or(1280);
                if !(320..=4096).contains(&max_width) {
                    return Err("max_width must be between 320 and 4096.".into());
                }
                let image = self.session.screenshot(max_width as u32)?;
                let text = format!(
                    "Screenshot {}×{} of a {}×{} remote screen. Use this image's pixel coordinates for mouse tools.",
                    image.width, image.height, image.coordinates.remote_width, image.coordinates.remote_height
                );
                Ok(ToolResult {
                    content: vec![
                        json!({"type": "image", "data": base64::engine::general_purpose::STANDARD.encode(image.png), "mimeType": "image/png"}),
                        json!({"type": "text", "text": text}),
                    ],
                    is_error: false,
                })
            }
            "click" => {
                let (x, y) = self.point(arguments, "x", "y")?;
                let button = button(
                    arguments
                        .get("button")
                        .and_then(Value::as_str)
                        .unwrap_or("left"),
                )?;
                let count = optional_integer(arguments, "count")?.unwrap_or(1);
                if !(1..=3).contains(&count) {
                    return Err("count must be 1, 2, or 3.".into());
                }
                self.session.input(|encoder| {
                    let mut payloads = encoder.pointer_moved(x, y);
                    let double_message = match button {
                        0 => 0x203,
                        1 => 0x206,
                        _ => 0x209,
                    };
                    if count >= 2 {
                        payloads.push(input::mouse_button(
                            double_message,
                            0,
                            input::packed_position(x as i16, y as i16),
                        ));
                    }
                    if count != 2 {
                        payloads.extend(encoder.pointer_button(button, true, Some((x, y))));
                        payloads.extend(encoder.pointer_button(button, false, Some((x, y))));
                    }
                    payloads
                })?;
                Ok(ToolResult::text(format!(
                    "Clicked at {x},{y} on the remote screen ({count} click(s))."
                )))
            }
            "move_mouse" => {
                let (x, y) = self.point(arguments, "x", "y")?;
                self.session.input(|encoder| encoder.pointer_moved(x, y))?;
                Ok(ToolResult::text(format!("Moved to {x},{y}.")))
            }
            "drag" => {
                let start = self.point(arguments, "from_x", "from_y")?;
                let end = self.point(arguments, "to_x", "to_y")?;
                let button = button(
                    arguments
                        .get("button")
                        .and_then(Value::as_str)
                        .unwrap_or("left"),
                )?;
                self.session.input(|encoder| {
                    let mut payloads = encoder.pointer_moved(start.0, start.1);
                    payloads.extend(encoder.pointer_button(button, true, Some(start)));
                    for step in 1..=12 {
                        let fraction = step as f64 / 12.0;
                        let x = start.0 + ((end.0 - start.0) as f64 * fraction) as i32;
                        let y = start.1 + ((end.1 - start.1) as f64 * fraction) as i32;
                        payloads.extend(encoder.pointer_moved(x, y));
                    }
                    payloads.extend(encoder.pointer_button(button, false, Some(end)));
                    payloads
                })?;
                Ok(ToolResult::text(format!(
                    "Dragged from {},{} to {},{}.",
                    start.0, start.1, end.0, end.1
                )))
            }
            "scroll" => {
                let direction = arguments
                    .get("direction")
                    .and_then(Value::as_str)
                    .unwrap_or("down");
                if direction != "up" && direction != "down" {
                    return Err("direction must be up or down.".into());
                }
                let amount = optional_integer(arguments, "amount")?.unwrap_or(3);
                if !(1..=50).contains(&amount) {
                    return Err("amount must be between 1 and 50.".into());
                }
                let position = match (arguments.get("x"), arguments.get("y")) {
                    (None, None) => Some(self.session.cursor_point().ok_or(
                        "No remote pointer position is known yet. Give x and y from the latest screenshot.",
                    )?),
                    (Some(_), Some(_)) => Some(self.point(arguments, "x", "y")?),
                    _ => return Err("Give both x and y, or omit both.".into()),
                };
                let lines = if direction == "up" {
                    amount as f64
                } else {
                    -(amount as f64)
                };
                self.session.input(|encoder| {
                    let mut payloads = position
                        .map(|(x, y)| encoder.pointer_moved(x, y))
                        .unwrap_or_default();
                    payloads.extend(encoder.scroll(lines));
                    payloads
                })?;
                Ok(ToolResult::text(format!("Scrolled {direction} {amount}.")))
            }
            "type_text" => {
                let text = required_string(arguments, "text")?;
                if text.is_empty() || text.chars().count() > 10_000 {
                    return Err("Give between 1 and 10,000 characters to type.".into());
                }
                let mut summary = keys::TypingSummary::default();
                self.session.input(|encoder| {
                    let (payloads, result) = keys::typing_payloads(encoder, text);
                    summary = result;
                    payloads
                })?;
                Ok(ToolResult::text(format!(
                    "Typed text ({} UTF-16 units skipped).",
                    summary.skipped_units
                )))
            }
            "press_keys" => {
                let text = required_string(arguments, "keys")?;
                if text.len() > 1000 {
                    return Err("The key sequence is too long.".into());
                }
                let chords = keys::parse(text)?;
                let repeat = optional_integer(arguments, "repeat")?.unwrap_or(1);
                if !(1..=100).contains(&repeat) {
                    return Err("repeat must be between 1 and 100.".into());
                }
                self.session
                    .input(|encoder| keys::payloads(encoder, &chords, repeat as u32))?;
                Ok(ToolResult::text(format!("Pressed {text}.")))
            }
            "remote_action" => {
                let action = match required_string(arguments, "action")? {
                    "ctrl_alt_delete" => Action::ControlAltDelete,
                    "lock_screen" => Action::LockScreen,
                    _ => return Err("action must be ctrl_alt_delete or lock_screen.".into()),
                };
                self.session.remote_action(action)?;
                Ok(ToolResult::text("Remote action sent."))
            }
            _ => Err(format!("Unknown tool {name}.").into()),
        }
    }

    fn point(&mut self, arguments: &Value, x: &str, y: &str) -> Result<(i32, i32)> {
        let x = required_integer(arguments, x)?;
        let y = required_integer(arguments, y)?;
        self.session.remote_point(x, y)
    }
}

fn response(id: Value, result: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

fn required_string<'a>(arguments: &'a Value, key: &str) -> Result<&'a str> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Give {key} as a string.").into())
}

fn required_integer(arguments: &Value, key: &str) -> Result<i64> {
    arguments
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("Give {key} as an integer.").into())
}

fn optional_integer(arguments: &Value, key: &str) -> Result<Option<i64>> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be an integer.").into()),
    }
}

fn button(name: &str) -> Result<u8> {
    match name {
        "left" => Ok(0),
        "right" => Ok(1),
        "middle" => Ok(2),
        _ => Err("button must be left, right, or middle.".into()),
    }
}

fn string(description: &str, values: Option<&[&str]>) -> Value {
    let mut schema = json!({"type": "string", "description": description});
    if let Some(values) = values {
        schema["enum"] = json!(values);
    }
    schema
}

fn integer(description: &str, minimum: Option<i64>) -> Value {
    let mut schema = json!({"type": "integer", "description": description});
    if let Some(minimum) = minimum {
        schema["minimum"] = json!(minimum);
    }
    schema
}

fn tool(
    name: &str,
    title: &str,
    description: &str,
    properties: &[(&str, Value)],
    required: &[&str],
    read_only: bool,
    destructive: bool,
) -> Value {
    let mut property_map = Map::new();
    for (key, schema) in properties {
        property_map.insert((*key).into(), schema.clone());
    }
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {"type": "object", "properties": property_map, "required": required, "additionalProperties": false},
        "annotations": {"title": title, "readOnlyHint": read_only, "destructiveHint": destructive, "openWorldHint": true}
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool("session_status", "Session status", "Shows connection state, remote dimensions, control permission, and screenshot readiness.", &[], &[], true, false),
        tool("connect", "Connect", "Connects to an authorized computer using its current UltraViewer ID and password.", &[
            ("partner_id", string("The numeric partner ID shown in UltraViewer.", None)),
            ("password", string("The current partner password. It is not saved by this server.", None)),
        ], &["partner_id", "password"], false, false),
        tool("disconnect", "Disconnect", "Ends the current remote session.", &[], &[], false, true),
        tool("screenshot", "Screenshot", "Returns the current remote screen as PNG. Mouse coordinates refer to this image's pixels.", &[
            ("max_width", integer("Scale down to at most this width, 320-4096. Default 1280.", Some(320))),
        ], &[], true, false),
        tool("click", "Click", "Clicks at a point in the latest screenshot.", &[
            ("x", integer("X in screenshot pixels.", Some(0))), ("y", integer("Y in screenshot pixels.", Some(0))),
            ("button", string("Mouse button; default left.", Some(&["left", "right", "middle"]))),
            ("count", integer("1, 2, or 3 clicks; default 1.", Some(1))),
        ], &["x", "y"], false, false),
        tool("move_mouse", "Move mouse", "Moves the pointer to a point in the latest screenshot.", &[
            ("x", integer("X in screenshot pixels.", Some(0))), ("y", integer("Y in screenshot pixels.", Some(0))),
        ], &["x", "y"], false, false),
        tool("drag", "Drag", "Drags between two points in the latest screenshot.", &[
            ("from_x", integer("Start X.", Some(0))), ("from_y", integer("Start Y.", Some(0))),
            ("to_x", integer("End X.", Some(0))), ("to_y", integer("End Y.", Some(0))),
            ("button", string("Mouse button; default left.", Some(&["left", "right", "middle"]))),
        ], &["from_x", "from_y", "to_x", "to_y"], false, false),
        tool("scroll", "Scroll", "Scrolls vertically, optionally at a screenshot point.", &[
            ("direction", string("Scroll direction.", Some(&["up", "down"]))),
            ("amount", integer("Wheel steps, 1-50; default 3.", Some(1))),
            ("x", integer("Optional screenshot X; requires y.", Some(0))), ("y", integer("Optional screenshot Y; requires x.", Some(0))),
        ], &["direction"], false, false),
        tool("type_text", "Type text", "Types Unicode text; newlines press Enter and tabs press Tab.", &[
            ("text", string("Text to type, up to 10,000 characters.", None)),
        ], &["text"], false, false),
        tool("press_keys", "Press keys", "Presses key chords such as ctrl+c, alt+f4, win+r, or enter. Separate chords with spaces.", &[
            ("keys", string("Key combination(s).", None)), ("repeat", integer("Times to press, 1-100; default 1.", Some(1))),
        ], &["keys"], false, false),
        tool("remote_action", "Remote action", "Sends Ctrl-Alt-Delete or locks the remote screen.", &[
            ("action", string("Remote action.", Some(&["ctrl_alt_delete", "lock_screen"]))),
        ], &["action"], false, true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_lists_tools_and_reports_disconnected_state() {
        let mut server = Server::default();
        let initialize: Value = serde_json::from_str(&server.handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#).unwrap()).unwrap();
        assert_eq!(initialize["result"]["protocolVersion"], "2025-03-26");
        let tools: Value = serde_json::from_str(
            &server
                .handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
                .unwrap(),
        )
        .unwrap();
        assert!(tools["result"]["tools"].as_array().unwrap().len() >= 10);
        let status: Value = serde_json::from_str(&server.handle_line(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"session_status","arguments":{}}}"#).unwrap()).unwrap();
        assert_eq!(status["result"]["isError"], false);
        assert!(status["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("disconnected"));
        let screenshot: Value = serde_json::from_str(&server.handle_line(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"screenshot","arguments":{}}}"#).unwrap()).unwrap();
        assert_eq!(screenshot["result"]["isError"], true);
    }

    #[test]
    fn rejects_unknown_methods_and_invalid_arguments() {
        let mut server = Server::default();
        let unknown: Value = serde_json::from_str(
            &server
                .handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"unknown"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
        let invalid: Value = serde_json::from_str(&server.handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"click","arguments":{"x":"bad","y":0}}}"#).unwrap()).unwrap();
        assert_eq!(invalid["result"]["isError"], true);
        assert!(invalid["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("integer"));
        assert!(server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
    }
}
