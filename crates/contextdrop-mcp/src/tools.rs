use std::fs::{self, File};
use std::io::Read;

use contextdrop_core::{
    compress, inspect_protection, protection_rule_catalog, CompressionConfig, InputFormat,
    Strategy, TokenizerKind,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub(crate) const DEFAULT_MAX_INPUT_BYTES: usize = 1_000_000;
pub(crate) const MAX_MCP_INPUT_BYTES: usize = 4_000_000;
const MAX_PATH_BYTES: usize = 4_096;
const DEFAULT_MAX_MATCHES: usize = 50;
const MAX_MATCHES: usize = 200;

pub(crate) const INSTRUCTIONS: &str = "\
ContextDrop compresses build, test, log, and JSON text locally with deterministic rules that \
protect diagnostic evidence. Before reading a large log, call compress_context_file with its \
path so the raw file does not enter context. For command output already in hand, call \
compress_context. Prefer the balanced strategy, use conservative for sensitive transcripts, \
and use aggressive only after reviewing the diff. Tool content is the compressed text; \
metrics, omissions, warnings, and preservation checks are in structuredContent. If \
fallback_used is true, ContextDrop returned the original unchanged.";

/// The successful result of one tool invocation.
#[derive(Debug)]
pub(crate) struct ToolSuccess {
    pub(crate) output: String,
    pub(crate) structured: Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompressTextArgs {
    text: String,
    #[serde(flatten)]
    options: CompressionOptions,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompressFileArgs {
    path: String,
    #[serde(flatten)]
    options: CompressionOptions,
}

#[derive(Debug, Deserialize)]
struct CompressionOptions {
    #[serde(default)]
    strategy: Strategy,
    #[serde(default)]
    input_format: InputFormat,
    #[serde(default)]
    preserve_patterns: Vec<String>,
    #[serde(default)]
    tokenizer: TokenizerKind,
    #[serde(default = "default_max_input_bytes")]
    max_input_bytes: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InspectArgs {
    text: String,
    #[serde(default)]
    preserve_patterns: Vec<String>,
    #[serde(default = "default_max_matches")]
    max_matches: usize,
}

fn default_max_input_bytes() -> usize {
    DEFAULT_MAX_INPUT_BYTES
}

fn default_max_matches() -> usize {
    DEFAULT_MAX_MATCHES
}

impl CompressionOptions {
    fn into_config(self) -> Result<CompressionConfig, String> {
        if !(1..=MAX_MCP_INPUT_BYTES).contains(&self.max_input_bytes) {
            return Err(format!(
                "max_input_bytes must be between 1 and {MAX_MCP_INPUT_BYTES}."
            ));
        }
        let config = CompressionConfig {
            strategy: self.strategy,
            input_format: self.input_format,
            preserve_patterns: self.preserve_patterns,
            tokenizer: self.tokenizer,
            max_input_bytes: self.max_input_bytes,
            ..CompressionConfig::default()
        };
        config.validate().map_err(|error| error.to_string())?;
        Ok(config)
    }
}

pub(crate) fn compress_text(arguments: Value) -> Result<ToolSuccess, String> {
    let arguments: CompressTextArgs =
        serde_json::from_value(arguments).map_err(|error| format!("Invalid arguments: {error}"))?;
    let config = arguments.options.into_config()?;
    if arguments.text.len() > config.max_input_bytes {
        return Err(format!(
            "text is {} bytes; the configured limit is {} bytes.",
            arguments.text.len(),
            config.max_input_bytes
        ));
    }

    let result = compress(&arguments.text, &config).map_err(|error| error.to_string())?;
    let result_value = serde_json::to_value(&result)
        .map_err(|error| format!("Could not serialize compression result: {error}"))?;
    Ok(ToolSuccess {
        output: result.output,
        structured: json!({
            "source": { "kind": "text" },
            "result": result_value,
        }),
    })
}

pub(crate) fn compress_file(arguments: Value) -> Result<ToolSuccess, String> {
    let arguments: CompressFileArgs =
        serde_json::from_value(arguments).map_err(|error| format!("Invalid arguments: {error}"))?;
    let config = arguments.options.into_config()?;
    let text = read_text_file(&arguments.path, config.max_input_bytes)?;
    let result = compress(&text, &config).map_err(|error| error.to_string())?;
    let result_value = serde_json::to_value(&result)
        .map_err(|error| format!("Could not serialize compression result: {error}"))?;
    Ok(ToolSuccess {
        output: result.output,
        structured: json!({
            "source": { "kind": "file", "path": arguments.path },
            "result": result_value,
        }),
    })
}

pub(crate) fn inspect(arguments: Value) -> Result<ToolSuccess, String> {
    let arguments: InspectArgs =
        serde_json::from_value(arguments).map_err(|error| format!("Invalid arguments: {error}"))?;
    if !(1..=MAX_MATCHES).contains(&arguments.max_matches) {
        return Err(format!("max_matches must be between 1 and {MAX_MATCHES}."));
    }
    if arguments.text.len() > MAX_MCP_INPUT_BYTES {
        return Err(format!(
            "text is {} bytes; the maximum is {MAX_MCP_INPUT_BYTES} bytes.",
            arguments.text.len()
        ));
    }

    let config = CompressionConfig {
        preserve_patterns: arguments.preserve_patterns,
        max_input_bytes: MAX_MCP_INPUT_BYTES,
        ..CompressionConfig::default()
    };
    config.validate().map_err(|error| error.to_string())?;

    let inspection = inspect_protection(
        &arguments.text,
        &config.preserve_patterns,
        arguments.max_matches,
    );
    let mut output = format!(
        "{} of {} lines matched protection rules.",
        inspection.protected_lines, inspection.total_lines
    );
    if inspection.truncated {
        output.push_str(&format!(
            " Match details are truncated to {} entries.",
            arguments.max_matches
        ));
    }
    output.push(' ');
    output.push_str(&inspection.note);

    let structured = serde_json::to_value(&inspection)
        .map_err(|error| format!("Could not serialize inspection result: {error}"))?;
    Ok(ToolSuccess { output, structured })
}

pub(crate) fn list_rules(_arguments: Value) -> Result<ToolSuccess, String> {
    let rules = protection_rule_catalog();
    let mut output = String::from("Built-in protection rules:\n");
    for rule in &rules {
        output.push_str("- ");
        output.push_str(rule.id);
        output.push_str(": ");
        output.push_str(rule.label);
        output.push('\n');
    }
    let structured = serde_json::to_value(&rules)
        .map_err(|error| format!("Could not serialize rules: {error}"))?;
    Ok(ToolSuccess { output, structured })
}

fn read_text_file(path: &str, limit: usize) -> Result<String, String> {
    if path.is_empty() {
        return Err("path cannot be empty.".into());
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(format!("path must be at most {MAX_PATH_BYTES} bytes."));
    }
    let display_path = truncate_path(path);
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot read file '{display_path}': {error}"))?;
    if !metadata.is_file() {
        return Err(format!("path '{display_path}' is not a regular file."));
    }
    if metadata.len() > limit as u64 {
        return Err(format!(
            "file '{display_path}' is {} bytes; the configured limit is {limit} bytes.",
            metadata.len()
        ));
    }

    let file =
        File::open(path).map_err(|error| format!("cannot read file '{display_path}': {error}"))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read file '{display_path}': {error}"))?;
    if bytes.len() > limit {
        return Err(format!(
            "file '{display_path}' grew beyond the configured limit of {limit} bytes while being read."
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        format!(
            "file '{display_path}' is not valid UTF-8 (first invalid byte at offset {}).",
            error.utf8_error().valid_up_to()
        )
    })
}

fn truncate_path(path: &str) -> String {
    const MAX_DISPLAY_CHARS: usize = 240;
    if path.chars().count() <= MAX_DISPLAY_CHARS {
        return path.to_string();
    }
    let mut truncated: String = path.chars().take(MAX_DISPLAY_CHARS).collect();
    truncated.push_str("...");
    truncated
}

pub(crate) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "compress_context",
            "title": "Compress context text",
            "description": "Compress UTF-8 build, test, log, or JSON text locally with deterministic evidence-preserving rules. Prefer this before putting a large log into context. Content is the compressed text; metrics, omissions, warnings, and preservation checks are in structuredContent. If no safe reduction is possible, the original is returned unchanged with fallback_used=true.",
            "inputSchema": compression_schema("text", json!({
                "type": "string",
                "description": "UTF-8 text to compress. The default size limit is 1,000,000 bytes."
            })),
            "annotations": annotations()
        }),
        json!({
            "name": "compress_context_file",
            "title": "Compress a context file",
            "description": "Read a local UTF-8 file and compress it without first returning the raw file to the agent. Use this for build logs, test output, CI logs, and large JSON files. This tool only reads the path you provide and never writes or executes it. The path resolves against the MCP server's working directory. The default size limit is 1,000,000 bytes.",
            "inputSchema": compression_schema("path", json!({
                "type": "string",
                "description": "Path to a readable UTF-8 regular file. Relative paths resolve against the MCP server's working directory."
            })),
            "annotations": annotations()
        }),
        json!({
            "name": "inspect_protection",
            "title": "Inspect protected evidence",
            "description": "Inspect which lines of pasted text match ContextDrop's built-in evidence rules and literal preserve patterns. This does not compress and does not prove semantic equivalence. Use it to understand why lines are protected or to verify a transcript before compression.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "UTF-8 text to inspect."
                    },
                    "preserve_patterns": {
                        "type": "array",
                        "description": "Literal, case-insensitive substrings to treat as protected evidence. Not regular expressions.",
                        "items": { "type": "string", "minLength": 1, "maxLength": 256 },
                        "maxItems": 100
                    },
                    "max_matches": {
                        "type": "integer",
                        "description": "Maximum number of matching lines to return. Counts cover the whole input.",
                        "minimum": 1,
                        "maximum": 200,
                        "default": 50
                    }
                },
                "required": ["text"],
                "additionalProperties": false
            },
            "annotations": annotations()
        }),
        json!({
            "name": "list_protection_rules",
            "title": "List protection rules",
            "description": "List ContextDrop's built-in protection rules by stable identifier and human-readable label. These rules decide which diagnostic lines must survive compression.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false
            },
            "annotations": annotations()
        }),
    ]
}

fn compression_schema(source_name: &str, source_schema: Value) -> Value {
    let mut properties = Map::new();
    properties.insert(source_name.to_string(), source_schema);
    properties.insert(
        "strategy".into(),
        json!({
            "type": "string",
            "description": "Compression intensity. Balanced is the default. Aggressive may remove non-protected context; review its diff.",
            "enum": ["conservative", "balanced", "aggressive"],
            "default": "balanced"
        }),
    );
    properties.insert(
        "input_format".into(),
        json!({
            "type": "string",
            "description": "Input classification. Automatic classification is conservative. Set a format when it is known.",
            "enum": ["auto", "plain-text", "terminal", "build-log", "test-output", "json", "structured-log"],
            "default": "auto"
        }),
    );
    properties.insert(
        "preserve_patterns".into(),
        json!({
            "type": "array",
            "description": "Literal, case-insensitive substrings that must survive compression. Not regular expressions.",
            "items": { "type": "string", "minLength": 1, "maxLength": 256 },
            "maxItems": 100
        }),
    );
    properties.insert(
        "tokenizer".into(),
        json!({
            "type": "string",
            "description": "Tokenizer used only for token metrics; compression is independent of this choice.",
            "enum": ["o200k_base", "cl100k_base", "estimate"],
            "default": "o200k_base"
        }),
    );
    properties.insert(
        "max_input_bytes".into(),
        json!({
            "type": "integer",
            "description": "Maximum accepted input size in bytes. The server allows at most 4,000,000 bytes.",
            "minimum": 1,
            "maximum": MAX_MCP_INPUT_BYTES,
            "default": DEFAULT_MAX_INPUT_BYTES
        }),
    );
    json!({
        "type": "object",
        "properties": properties,
        "required": [source_name],
        "additionalProperties": false
    })
}

fn annotations() -> Value {
    json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_args(text: &str) -> Value {
        json!({ "text": text, "tokenizer": "estimate" })
    }

    #[test]
    fn compresses_text_and_returns_structured_metrics() {
        let outcome = compress_text(text_args(
            "Compiling app\nCompiling app\nCompiling app\nerror: failed\n",
        ))
        .expect("compression should succeed");
        assert!(outcome.output.contains("error: failed"));
        assert_eq!(
            outcome.structured["result"]["strategy"],
            Value::String("balanced".into())
        );
        assert_eq!(
            outcome.structured["source"]["kind"],
            Value::String("text".into())
        );
    }

    #[test]
    fn rejects_input_over_the_tool_limit() {
        let error = compress_text(json!({
            "text": "a",
            "max_input_bytes": MAX_MCP_INPUT_BYTES + 1,
            "tokenizer": "estimate"
        }))
        .expect_err("over-limit configuration must fail");
        assert!(error.contains("max_input_bytes"));
    }

    #[test]
    fn reads_utf8_file_and_omits_repetition() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("build.log");
        fs::write(
            &path,
            "Compiling app\nCompiling app\nCompiling app\nerror: failed\n",
        )
        .expect("write fixture");
        let outcome = compress_file(json!({
            "path": path,
            "tokenizer": "estimate"
        }))
        .expect("file compression should succeed");
        assert!(outcome.output.contains("error: failed"));
        assert_eq!(
            outcome.structured["source"]["kind"],
            Value::String("file".into())
        );
    }

    #[test]
    fn rejects_missing_and_non_utf8_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let missing = directory.path().join("missing.log");
        let error = compress_file(json!({ "path": missing })).expect_err("missing file must fail");
        assert!(error.contains("cannot read file"));

        let invalid = directory.path().join("invalid.log");
        fs::write(&invalid, [0xff, 0xfe, 0x00]).expect("write invalid UTF-8");
        let error = compress_file(json!({ "path": invalid })).expect_err("invalid UTF-8 must fail");
        assert!(error.contains("not valid UTF-8"));
    }

    #[test]
    fn inspects_protected_lines() {
        let outcome = inspect(json!({
            "text": "compiling\n --> src/main.rs:10:5\nerror: failed\n"
        }))
        .expect("inspection should succeed");
        assert!(outcome.output.contains("2 of 3 lines"));
        assert!(outcome.structured["matches"].as_array().is_some());
    }

    #[test]
    fn lists_real_protection_rules() {
        let outcome = list_rules(json!({})).expect("rules should load");
        assert!(outcome.output.contains("error"));
        assert!(outcome.structured.as_array().expect("rules array").len() > 5);
    }
}
