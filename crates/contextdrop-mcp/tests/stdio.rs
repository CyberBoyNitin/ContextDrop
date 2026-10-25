use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn run_session(messages: &[Value]) -> (String, String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_contextdrop-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn contextdrop-mcp");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        for message in messages {
            serde_json::to_writer(&mut *stdin, message).expect("write JSON message");
            stdin.write_all(b"\n").expect("write newline");
        }
    }
    let output = child.wait_with_output().expect("wait for server");
    (
        String::from_utf8(output.stdout).expect("UTF-8 stdout"),
        String::from_utf8(output.stderr).expect("UTF-8 stderr"),
        output.status.success(),
    )
}

fn responses(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("every stdout line is JSON"))
        .collect()
}

fn legacy_messages() -> Vec<Value> {
    vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "contextdrop-test", "version": "1.0" }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "compress_context",
                "arguments": {
                    "text": "Compiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nerror: failed\n",
                    "strategy": "balanced",
                    "tokenizer": "estimate"
                }
            }
        }),
    ]
}

#[test]
fn legacy_stdio_session_lists_and_calls_tools_without_stdout_noise() {
    let (stdout, stderr, success) = run_session(&legacy_messages());
    assert!(success, "server exited unsuccessfully: {stderr}");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");

    let responses = responses(&stdout);
    assert_eq!(
        responses.len(),
        3,
        "one response per request, no notification response"
    );
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(responses[1]["id"], 2);
    assert!(responses[1]["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .any(|tool| tool["name"] == "compress_context_file"));
    assert_eq!(responses[2]["id"], 3);
    assert_eq!(responses[2]["result"]["isError"], false);
    let compressed = responses[2]["result"]["content"][0]["text"]
        .as_str()
        .expect("compressed text");
    assert!(compressed.contains("error: failed"));
    assert_eq!(
        responses[2]["result"]["structuredContent"]["result"]["preservation"]["passed"],
        true
    );
}

#[test]
fn file_tool_reads_a_real_file_without_returning_the_raw_file_first() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("build.log");
    std::fs::write(
        &path,
        "Compiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nCompiling app\nerror: failed\n",
    )
    .expect("write fixture");

    let messages = vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": "2025-11-25" }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "compress_context_file",
                "arguments": {
                    "path": path.to_string_lossy(),
                    "tokenizer": "estimate"
                }
            }
        }),
    ];
    let (stdout, stderr, success) = run_session(&messages);
    assert!(success, "server exited unsuccessfully: {stderr}");
    let responses = responses(&stdout);
    assert_eq!(responses[1]["result"]["isError"], false);
    assert_eq!(
        responses[1]["result"]["structuredContent"]["source"]["kind"],
        "file"
    );
    assert!(
        responses[1]["result"]["structuredContent"]["result"]["metrics"]["original_bytes"]
            .as_u64()
            .expect("original bytes")
            > responses[1]["result"]["structuredContent"]["result"]["metrics"]["output_bytes"]
                .as_u64()
                .expect("output bytes")
    );
}

#[test]
fn modern_discovery_and_tool_call_use_the_2026_07_28_shape() {
    let messages = vec![
        json!({
            "jsonrpc": "2.0",
            "id": "discover",
            "method": "server/discover",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1" },
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "list",
            "method": "tools/list",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "call",
            "method": "tools/call",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                },
                "name": "compress_context",
                "arguments": {
                    "text": "error: failed\nerror: failed\n",
                    "tokenizer": "estimate"
                }
            }
        }),
    ];
    let (stdout, stderr, success) = run_session(&messages);
    assert!(success, "server exited unsuccessfully: {stderr}");
    let responses = responses(&stdout);
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["result"]["resultType"], "complete");
    assert_eq!(responses[0]["result"]["ttlMs"], 300000);
    assert_eq!(responses[1]["result"]["resultType"], "complete");
    assert_eq!(responses[2]["result"]["resultType"], "complete");
    assert_eq!(responses[2]["result"]["isError"], false);
}

#[test]
fn unsupported_modern_version_fails_without_falling_back_to_legacy() {
    let messages = vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "1999-01-01"
            }
        }
    })];
    let (stdout, stderr, success) = run_session(&messages);
    assert!(success, "server exited unsuccessfully: {stderr}");
    let responses = responses(&stdout);
    assert_eq!(responses[0]["error"]["code"], -32022);
    assert_eq!(responses[0]["error"]["data"]["supported"][0], "2026-07-28");
}
