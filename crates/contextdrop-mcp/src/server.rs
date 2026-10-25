use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::tools;

pub(crate) const MODERN_PROTOCOL_VERSION: &str = "2026-07-28";
pub(crate) const LATEST_LEGACY_PROTOCOL_VERSION: &str = "2025-11-25";
const LEGACY_PROTOCOL_VERSIONS: [&str; 4] =
    ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const PROTOCOL_VERSION_META_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const CLIENT_CAPABILITIES_META_KEY: &str = "io.modelcontextprotocol/clientCapabilities";
const CLIENT_INFO_META_KEY: &str = "io.modelcontextprotocol/clientInfo";
const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;
const CACHE_TTL_MS: u64 = 300_000;

const PARSE_ERROR: i64 = -32_700;
const INVALID_REQUEST: i64 = -32_600;
const METHOD_NOT_FOUND: i64 = -32_601;
const INVALID_PARAMS: i64 = -32_602;
const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32_022;

/// Serve one MCP stdio session.
///
/// The loop reads newline-delimited JSON-RPC messages and writes only JSON-RPC
/// messages to `writer`. Diagnostics and failures belong on stderr, never on
/// this stream.
pub fn serve<R: BufRead, W: Write>(mut reader: R, mut writer: W) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }
        if line.len() > MAX_MESSAGE_BYTES {
            let response = error_response(
                Value::Null,
                RpcError::new(INVALID_REQUEST, "MCP message exceeds the 32 MiB limit."),
            );
            write_message(&mut writer, &response)?;
            continue;
        }

        let message = line.trim_end_matches(['\r', '\n']);
        if let Some(response) = handle_line(message) {
            if write_message(&mut writer, &response).is_err() {
                return Ok(());
            }
        }
    }
}

fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, message)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn handle_line(line: &str) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(message) => message,
        Err(error) => {
            let detail = format!("Invalid JSON-RPC message: {error}");
            return Some(error_response(
                Value::Null,
                RpcError::new(PARSE_ERROR, detail),
            ));
        }
    };
    handle_message(message)
}

fn handle_message(message: Value) -> Option<Value> {
    let object = match message.as_object() {
        Some(object) => object,
        None => {
            return Some(error_response(
                Value::Null,
                RpcError::new(INVALID_REQUEST, "JSON-RPC message must be an object."),
            ))
        }
    };

    let has_id = object.contains_key("id");
    let id = object.get("id").cloned();
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return if has_id {
            Some(error_response(
                id.unwrap_or(Value::Null),
                RpcError::new(INVALID_REQUEST, "jsonrpc must be \"2.0\"."),
            ))
        } else {
            None
        };
    }

    let method = match object.get("method").and_then(Value::as_str) {
        Some(method) => method,
        None => {
            // A client response to a server request is valid input but this
            // server never sends requests. Ignore it.
            if has_id && (object.contains_key("result") || object.contains_key("error")) {
                return None;
            }
            return if has_id {
                Some(error_response(
                    id.unwrap_or(Value::Null),
                    RpcError::new(INVALID_REQUEST, "Request is missing a string method."),
                ))
            } else {
                None
            };
        }
    };

    let params = object
        .get("params")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    let modern = match modern_request(&params) {
        Ok(modern) => modern,
        Err(error) => {
            return if has_id {
                Some(error_response(id.unwrap_or(Value::Null), error))
            } else {
                None
            }
        }
    };

    let result = dispatch(method, &params, modern);
    if !has_id {
        return None;
    }
    let id = id.unwrap_or(Value::Null);
    Some(match result {
        Ok(mut result) => {
            if modern {
                add_server_meta(&mut result);
            }
            json!({ "jsonrpc": "2.0", "id": id, "result": result })
        }
        Err(error) => error_response(id, error),
    })
}

fn add_server_meta(result: &mut Value) {
    let Some(result) = result.as_object_mut() else {
        return;
    };
    let meta = result
        .entry("_meta")
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(meta) = meta.as_object_mut() {
        meta.insert("io.modelcontextprotocol/serverInfo".into(), server_info());
    }
}

fn modern_request(params: &Value) -> Result<bool, RpcError> {
    let Some(meta) = params.get("_meta") else {
        return Ok(false);
    };
    let Some(meta) = meta.as_object() else {
        return Err(RpcError::new(
            INVALID_PARAMS,
            "params._meta must be an object.",
        ));
    };
    let Some(version) = meta.get(PROTOCOL_VERSION_META_KEY) else {
        // Legacy clients such as OpenCode may use _meta for request
        // correlation without opting into the modern protocol.
        return Ok(false);
    };
    let Some(version) = version.as_str() else {
        return Err(RpcError::new(
            INVALID_PARAMS,
            "params._meta protocol version must be a string.",
        ));
    };
    if version == MODERN_PROTOCOL_VERSION {
        let Some(capabilities) = meta.get(CLIENT_CAPABILITIES_META_KEY) else {
            return Err(RpcError::new(
                INVALID_PARAMS,
                "params._meta is missing io.modelcontextprotocol/clientCapabilities.",
            ));
        };
        if !capabilities.is_object() {
            return Err(RpcError::new(
                INVALID_PARAMS,
                "io.modelcontextprotocol/clientCapabilities must be an object.",
            ));
        }
        if let Some(client_info) = meta.get(CLIENT_INFO_META_KEY) {
            if !client_info.is_object() {
                return Err(RpcError::new(
                    INVALID_PARAMS,
                    "io.modelcontextprotocol/clientInfo must be an object.",
                ));
            }
        }
        Ok(true)
    } else {
        Err(RpcError::new(
            UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version.",
        )
        .with_data(json!({
            "supported": [MODERN_PROTOCOL_VERSION],
            "requested": version,
        })))
    }
}

fn dispatch(method: &str, params: &Value, modern: bool) -> Result<Value, RpcError> {
    match method {
        "initialize" => initialize(params),
        "server/discover" => {
            if !modern {
                return Err(RpcError::new(
                    METHOD_NOT_FOUND,
                    "server/discover requires the 2026-07-28 protocol metadata.",
                ));
            }
            Ok(discover_result())
        }
        "notifications/initialized" | "notifications/cancelled" => Ok(Value::Null),
        "ping" => Ok(if modern {
            json!({ "resultType": "complete" })
        } else {
            json!({})
        }),
        "tools/list" => Ok(list_tools_result(modern)),
        "tools/call" => call_tool(params, modern),
        _ => Err(RpcError::new(
            METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )),
    }
}

fn initialize(params: &Value) -> Result<Value, RpcError> {
    let params = params
        .as_object()
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "initialize params must be an object."))?;
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(LATEST_LEGACY_PROTOCOL_VERSION);
    let protocol_version = if LEGACY_PROTOCOL_VERSIONS.contains(&requested) {
        requested
    } else {
        LATEST_LEGACY_PROTOCOL_VERSION
    };

    Ok(json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": server_info(),
        "instructions": tools::INSTRUCTIONS
    }))
}

fn discover_result() -> Value {
    json!({
        "resultType": "complete",
        "supportedVersions": [MODERN_PROTOCOL_VERSION],
        "capabilities": { "tools": {} },
        "_meta": {
            "io.modelcontextprotocol/serverInfo": server_info()
        },
        "instructions": tools::INSTRUCTIONS,
        "ttlMs": CACHE_TTL_MS,
        "cacheScope": "public"
    })
}

fn list_tools_result(modern: bool) -> Value {
    let mut result = json!({ "tools": tools::definitions() });
    if modern {
        result["resultType"] = json!("complete");
        result["ttlMs"] = json!(CACHE_TTL_MS);
        result["cacheScope"] = json!("public");
    }
    result
}

fn call_tool(params: &Value, modern: bool) -> Result<Value, RpcError> {
    let params = params
        .as_object()
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "tools/call params must be an object."))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "tools/call requires a string name."))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Ok(tool_error("Tool arguments must be a JSON object.", modern));
    }

    let outcome = match name {
        "compress_context" => tools::compress_text(arguments),
        "compress_context_file" => tools::compress_file(arguments),
        "inspect_protection" => tools::inspect(arguments),
        "list_protection_rules" => tools::list_rules(arguments),
        _ => {
            return Err(RpcError::new(
                INVALID_PARAMS,
                format!("Unknown tool: {name}"),
            ))
        }
    };

    Ok(match outcome {
        Ok(success) => tool_success(success.output, success.structured, modern),
        Err(error) => tool_error(&error, modern),
    })
}

fn tool_success(output: String, structured: Value, modern: bool) -> Value {
    let mut result = json!({
        "content": [{ "type": "text", "text": output }],
        "structuredContent": structured,
        "isError": false
    });
    if modern {
        result["resultType"] = json!("complete");
    }
    result
}

fn tool_error(message: &str, modern: bool) -> Value {
    let mut result = json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    });
    if modern {
        result["resultType"] = json!("complete");
    }
    result
}

fn server_info() -> Value {
    json!({
        "name": "contextdrop",
        "title": "ContextDrop",
        "version": contextdrop_core::VERSION,
        "description": "Deterministic, evidence-preserving context compression"
    })
}

fn error_response(id: Value, error: RpcError) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": error.into_json()
    })
}

#[derive(Debug)]
struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    fn into_json(self) -> Value {
        let mut error = json!({
            "code": self.code,
            "message": self.message,
        });
        if let Some(data) = self.data {
            error["data"] = data;
        }
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(value: Value) -> Option<Value> {
        handle_message(value)
    }

    fn call(request_value: Value) -> Value {
        request(request_value).expect("request must produce a response")
    }

    fn id_value(response: &Value) -> Value {
        response["id"].clone()
    }

    #[test]
    fn legacy_initialize_negotiates_a_legacy_version() {
        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }
        }));
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
        assert_eq!(response["result"]["serverInfo"]["name"], "contextdrop");
    }

    #[test]
    fn modern_discover_and_tools_calls_include_result_type() {
        let discover = call(json!({
            "jsonrpc": "2.0",
            "id": "discover-1",
            "method": "server/discover",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": MODERN_PROTOCOL_VERSION,
                    "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1" },
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }));
        assert_eq!(discover["result"]["resultType"], "complete");
        assert_eq!(
            discover["result"]["supportedVersions"][0],
            MODERN_PROTOCOL_VERSION
        );

        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": MODERN_PROTOCOL_VERSION,
                    "io.modelcontextprotocol/clientCapabilities": {}
                },
                "name": "compress_context",
                "arguments": {
                    "text": "Compiling app\nCompiling app\nerror: failed\n",
                    "tokenizer": "estimate"
                }
            }
        }));
        assert_eq!(response["result"]["resultType"], "complete");
        assert_eq!(response["result"]["isError"], false);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .expect("content text")
            .contains("error: failed"));
        assert_eq!(
            response["result"]["structuredContent"]["result"]["strategy"],
            "balanced"
        );
    }

    #[test]
    fn unsupported_modern_version_is_rejected_with_supported_list() {
        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/list",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "1900-01-01"
                }
            }
        }));
        assert_eq!(response["error"]["code"], UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(
            response["error"]["data"]["supported"][0],
            MODERN_PROTOCOL_VERSION
        );
        assert_eq!(response["error"]["data"]["requested"], "1900-01-01");
    }

    #[test]
    fn modern_request_requires_client_capabilities() {
        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/list",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": MODERN_PROTOCOL_VERSION
                }
            }
        }));
        assert_eq!(response["error"]["code"], INVALID_PARAMS);
        assert!(response["error"]["message"]
            .as_str()
            .expect("message")
            .contains("clientCapabilities"));
    }

    #[test]
    fn modern_results_include_server_identity_metadata() {
        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/list",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": MODERN_PROTOCOL_VERSION,
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }));
        assert_eq!(
            response["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "contextdrop"
        );
        assert_eq!(response["result"]["resultType"], "complete");
    }

    #[test]
    fn legacy_server_discover_falls_back_as_method_not_found() {
        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "server/discover",
            "params": {}
        }));
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn unknown_tool_is_a_protocol_error() {
        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": { "name": "not_a_tool", "arguments": {} }
        }));
        assert_eq!(response["error"]["code"], INVALID_PARAMS);
        assert!(response["error"]["message"]
            .as_str()
            .expect("message")
            .contains("Unknown tool"));
    }

    #[test]
    fn invalid_tool_arguments_are_tool_errors() {
        let response = call(json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": { "name": "compress_context", "arguments": {} }
        }));
        assert_eq!(response["result"]["isError"], true);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .expect("error text")
            .contains("Invalid arguments"));
    }

    #[test]
    fn notifications_never_receive_responses() {
        assert!(request(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .is_none());
    }

    #[test]
    fn invalid_json_is_a_parse_error_with_null_id() {
        let response = handle_line("not json").expect("parse error response");
        assert_eq!(response["error"]["code"], PARSE_ERROR);
        assert!(response["id"].is_null());
    }

    #[test]
    fn serve_processes_multiple_lines_and_exits_at_eof() {
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n"
        );
        let mut output = Vec::new();
        serve(io::Cursor::new(input.as_bytes()), &mut output).expect("serve");
        let lines = String::from_utf8(output).expect("UTF-8 output");
        let messages: Vec<Value> = lines
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid JSON line"))
            .collect();
        assert_eq!(messages.len(), 2);
        assert_eq!(id_value(&messages[0]), 1);
        assert_eq!(id_value(&messages[1]), 2);
        assert!(messages[1]["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .any(|tool| tool["name"] == "compress_context_file"));
    }
}
