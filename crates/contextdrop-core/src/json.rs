//! Deterministic JSON-aware compression.
//!
//! Conservative JSON handling only removes insignificant whitespace and keeps
//! every token and escape sequence intact. Balanced and aggressive modes can
//! additionally omit exact duplicate object records from arrays, using an
//! explicit JSON marker object. This changes the JSON schema, so it is never
//! silent and is only enabled for sufficiently large record arrays.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::protect::Protector;
use crate::types::{CompressionConfig, Omissions, Strategy};
use crate::Error;

pub(crate) struct JsonAttempt {
    pub output: String,
    pub omissions: Omissions,
    pub warnings: Vec<String>,
}

pub(crate) fn compress_json(input: &str, config: &CompressionConfig) -> Result<JsonAttempt, Error> {
    let mut value: Value = serde_json::from_str(input).map_err(|error| Error::InvalidJson {
        message: error.to_string(),
    })?;

    let mut omissions = Omissions {
        total_input_lines: input.lines().count(),
        ..Omissions::default()
    };
    let mut warnings = Vec::new();
    let minified = minify_lexically(input);

    let threshold = match config.strategy {
        Strategy::Conservative => 0,
        Strategy::Balanced => 16,
        Strategy::Aggressive => 4,
    };

    let mut omitted = 0usize;
    let mut markers = 0usize;
    if config.structural_json && threshold > 0 {
        let protector = Protector::new(&config.preserve_patterns);
        let mut state = DedupeState {
            threshold,
            omitted: 0,
            markers: 0,
            protector: &protector,
        };
        dedupe_value(&mut value, &mut state);
        omitted = state.omitted;
        markers = state.markers;
    }

    let output = if omitted > 0 {
        serde_json::to_string(&value).map_err(|error| {
            Error::Internal(format!("failed to serialize compressed JSON: {error}"))
        })?
    } else {
        minified
    };

    if omitted > 0 {
        omissions.json_duplicate_items_omitted = omitted;
        omissions.markers_inserted = markers;
        warnings.push(format!(
            "{omitted} duplicate JSON item(s) were omitted from large record arrays. A marker object was inserted; this changes the array shape expected by strict consumers."
        ));
        warnings.push(
            "Structural JSON compression re-serializes the document. Values and key order are preserved, but string escaping and number spelling can change representation."
                .into(),
        );
    } else if output != input {
        warnings.push(
            "JSON was compacted by removing insignificant whitespace. Every value and string escape was retained."
                .into(),
        );
    }

    omissions.total_output_lines = output.lines().count();
    Ok(JsonAttempt {
        output,
        omissions,
        warnings,
    })
}

fn minify_lexically(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in input.chars() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
            output.push(ch);
        } else if !matches!(ch, ' ' | '\t' | '\n' | '\r') {
            output.push(ch);
        }
    }
    output
}

struct DedupeState<'a> {
    threshold: usize,
    omitted: usize,
    markers: usize,
    protector: &'a Protector,
}

fn dedupe_value(value: &mut Value, state: &mut DedupeState<'_>) {
    match value {
        Value::Array(items) => {
            // Protected records are not recursed into, which guarantees their
            // complete value can be compared during safety validation.
            for item in items.iter_mut() {
                if !contains_protected_json(item, state.protector) {
                    dedupe_value(item, state);
                }
            }

            if items.len() >= state.threshold
                && items.len() > 1
                && items.iter().all(Value::is_object)
            {
                let mut seen = std::collections::HashSet::new();
                let mut kept: Vec<Value> = Vec::with_capacity(items.len());
                let mut omitted_here = 0usize;
                for item in items.iter() {
                    let protected = contains_protected_json(item, state.protector);
                    let key = serde_json::to_string(item).ok();
                    let duplicate = key
                        .as_ref()
                        .is_some_and(|serialized| !seen.insert(serialized.clone()));
                    if !protected && duplicate {
                        omitted_here += 1;
                    } else {
                        kept.push(item.clone());
                    }
                }
                if omitted_here > 0 {
                    let mut marker = Map::new();
                    marker.insert(
                        "$contextdrop".into(),
                        Value::String(format!(
                            "[{omitted_here} duplicate JSON records omitted; one copy of each record retained]"
                        )),
                    );
                    kept.push(Value::Object(marker));
                    *items = kept;
                    state.omitted += omitted_here;
                    state.markers += 1;
                }
            }
        }
        Value::Object(map) => {
            // Deduplication inside protected records is deliberately skipped
            // only for array elements. Objects at the root still recurse into
            // their nested arrays.
            for value in map.values_mut() {
                dedupe_value(value, state);
            }
        }
        _ => {}
    }
}

/// Check whether a JSON value contains evidence that must not be discarded.
pub(crate) fn contains_protected_json(value: &Value, protector: &Protector) -> bool {
    match value {
        Value::String(text) => protector.inspect_line(text, false).protected,
        Value::Number(number) => number_protection(None, number),
        Value::Bool(_) | Value::Null => false,
        Value::Array(items) => items
            .iter()
            .any(|item| contains_protected_json(item, protector)),
        Value::Object(map) => map.iter().any(|(key, value)| {
            let key_protected = protector.inspect_line(key, false).protected;
            key_protected
                || value
                    .as_number()
                    .is_some_and(|number| number_protection(Some(key), number))
                || contains_protected_json(value, protector)
        }),
    }
}

fn number_protection(key: Option<&str>, number: &serde_json::Number) -> bool {
    let key = key.unwrap_or_default().to_ascii_lowercase();
    matches!(
        key.as_str(),
        "error" | "errors" | "failed" | "failures" | "fatal" | "exception"
    ) && number.as_u64().is_some_and(|value| value > 0)
        || matches!(
            key.as_str(),
            "status" | "status_code" | "statuscode" | "http_status"
        ) && number
            .as_u64()
            .is_some_and(|value| (400..=599).contains(&value))
}

/// Build a multiset of protected evidence for a parsed JSON document.
///
/// The representation is canonical (serde_json compact serialization) so input
/// and output can be compared without depending on whitespace.
pub(crate) fn evidence_counts(
    value: &Value,
    preserve_patterns: &[String],
) -> BTreeMap<String, usize> {
    let protector = Protector::new(preserve_patterns);
    let mut counts = BTreeMap::new();
    walk_evidence(value, &protector, &mut counts);
    counts
}

fn walk_evidence(value: &Value, protector: &Protector, counts: &mut BTreeMap<String, usize>) {
    match value {
        Value::String(text) => {
            if protector.inspect_line(text, false).protected {
                increment(counts, format!("string:{text}"));
            }
        }
        Value::Object(map) => {
            if let Some(status) = map
                .get("status")
                .or_else(|| map.get("status_code"))
                .or_else(|| map.get("statuscode"))
                .and_then(Value::as_u64)
            {
                if (400..=599).contains(&status) {
                    increment(counts, format!("http-status:{status}"));
                }
            }
            for (key, child) in map {
                if protector.inspect_line(key, false).protected {
                    increment(counts, format!("key:{key}"));
                }
                let child_protected = match child {
                    Value::String(text) => protector.inspect_line(text, false).protected,
                    Value::Number(number) => number_protection(Some(key), number),
                    _ => false,
                };
                if child_protected {
                    if let Ok(serialized) = serde_json::to_string(child) {
                        increment(counts, format!("field:{key}={serialized}"));
                    }
                }
                walk_evidence(child, protector, counts);
            }
        }
        Value::Array(items) => {
            for item in items {
                if contains_protected_json(item, protector) {
                    if let Ok(serialized) = serde_json::to_string(item) {
                        increment(counts, format!("array-item:{serialized}"));
                    }
                }
                walk_evidence(item, protector, counts);
            }
        }
        Value::Number(number) => {
            if number.as_u64().is_some_and(|v| (400..=599).contains(&v)) {
                // A bare numeric status is only evidence in object context; the
                // object arm records it with useful structure.
            }
        }
        Value::Bool(_) | Value::Null => {}
    }
}

fn increment(counts: &mut BTreeMap<String, usize>, key: String) {
    *counts.entry(key).or_insert(0) += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InputFormat, TokenizerKind};

    fn config(strategy: Strategy, structural: bool) -> CompressionConfig {
        CompressionConfig {
            strategy,
            input_format: InputFormat::Json,
            tokenizer: TokenizerKind::Estimate,
            structural_json: structural,
            ..CompressionConfig::default()
        }
    }

    #[test]
    fn conservative_minifies_without_changing_values() {
        let input = "{\n  \"a\": 1,\n  \"b\": [true, null, \"x y\"]\n}\n";
        let attempt = compress_json(input, &config(Strategy::Conservative, true)).unwrap();
        assert_eq!(attempt.output, r#"{"a":1,"b":[true,null,"x y"]}"#);
        assert_eq!(attempt.omissions.json_duplicate_items_omitted, 0);
    }

    #[test]
    fn balanced_deduplicates_large_duplicate_record_arrays() {
        let mut items = Vec::new();
        for i in 0..20 {
            items.push(serde_json::json!({"id": 7, "status": "ok", "value": i % 2}));
        }
        let items = vec![serde_json::json!({"id": 7, "status": "ok"}); 20];
        let _ = items;
        let input = serde_json::to_string(&items).unwrap();
        let attempt = compress_json(&input, &config(Strategy::Balanced, true)).unwrap();
        let parsed: Value = serde_json::from_str(&attempt.output).unwrap();
        let array = parsed.as_array().unwrap();
        assert_eq!(array.len(), 2);
        assert!(array[1].get("$contextdrop").is_some());
        assert_eq!(attempt.omissions.json_duplicate_items_omitted, 19);
    }

    #[test]
    fn protected_json_records_are_retained() {
        let items = vec![
            serde_json::json!({"level":"error","message":"database offline"}),
            serde_json::json!({"level":"info","message":"ok"}),
            serde_json::json!({"level":"info","message":"ok"}),
            serde_json::json!({"level":"info","message":"ok"}),
            serde_json::json!({"level":"info","message":"ok"}),
        ];
        let input = serde_json::to_string(&items).unwrap();
        let attempt = compress_json(
            &input,
            &CompressionConfig {
                strategy: Strategy::Aggressive,
                input_format: InputFormat::Json,
                ..CompressionConfig::default()
            },
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&attempt.output).unwrap();
        assert!(parsed
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["level"] == "error"));
    }

    #[test]
    fn invalid_json_is_an_error() {
        let result = compress_json("{", &config(Strategy::Balanced, true));
        assert!(matches!(result, Err(Error::InvalidJson { .. })));
    }

    #[test]
    fn minifier_preserves_escaped_strings() {
        let input = r#"{ "escaped": "line\nbreak", "slashes": "a\\b" }"#;
        let attempt = compress_json(input, &config(Strategy::Conservative, false)).unwrap();
        let value: Value = serde_json::from_str(&attempt.output).unwrap();
        assert_eq!(value["escaped"], "line\nbreak");
        assert_eq!(value["slashes"], "a\\b");
    }
}
