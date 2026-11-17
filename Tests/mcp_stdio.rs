use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn compiled_binary_serves_mcp_over_stdio_and_exits_on_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ultraviewer-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"session_status\",\"arguments\":{}}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"not_a_tool\",\"arguments\":{}}}\n"
    ).as_bytes()).unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(messages.len(), 4);
    assert_eq!(
        messages[0]["result"]["serverInfo"]["name"],
        "ultraviewer-mcp"
    );
    assert!(messages[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "screenshot"));
    assert_eq!(messages[2]["result"]["isError"], false);
    assert!(messages[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("disconnected"));
    assert_eq!(messages[3]["error"]["code"], -32602);
}
