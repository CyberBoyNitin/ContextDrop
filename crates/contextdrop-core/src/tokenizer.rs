use tiktoken_rs::{cl100k_base_singleton, o200k_base_singleton};

use crate::types::{TokenMeasurement, TokenMeasurementKind, TokenizerKind};

#[derive(Debug, Clone)]
pub(crate) struct TokenCount {
    pub count: usize,
    pub measurement: TokenMeasurement,
}

/// Count tokens locally with the selected tokenizer.
///
/// tiktoken-rs embeds BPE ranks in the binary. No network access is performed
/// and no text leaves the process.
pub(crate) fn count(text: &str, kind: TokenizerKind) -> TokenCount {
    match kind {
        TokenizerKind::O200kBase => {
            let bpe = o200k_base_singleton();
            let count = bpe.encode_ordinary(text).len();
            TokenCount {
                count,
                measurement: TokenMeasurement {
                    kind: TokenMeasurementKind::Exact,
                    name: "o200k_base".into(),
                    label: "Exact o200k_base tokens".into(),
                    method: "tiktoken-rs BPE".into(),
                    note: "Exact tokenizer count for the o200k_base encoding. Chat framing, tool schemas, and provider-specific overhead are not included.".into(),
                },
            }
        }
        TokenizerKind::Cl100kBase => {
            let bpe = cl100k_base_singleton();
            let count = bpe.encode_ordinary(text).len();
            TokenCount {
                count,
                measurement: TokenMeasurement {
                    kind: TokenMeasurementKind::Exact,
                    name: "cl100k_base".into(),
                    label: "Exact cl100k_base tokens".into(),
                    method: "tiktoken-rs BPE".into(),
                    note: "Exact tokenizer count for the cl100k_base encoding. Chat framing, tool schemas, and provider-specific overhead are not included.".into(),
                },
            }
        }
        TokenizerKind::Estimate => estimated(text),
    }
}

/// Conservative character-composition estimate.
///
/// ASCII text is assumed to average four characters per token, while non-ASCII
/// characters are assumed to consume roughly one token each. This is
/// deliberately labelled as an estimate and is never presented as exact.
fn estimated(text: &str) -> TokenCount {
    let mut ascii = 0usize;
    let mut non_ascii = 0usize;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii += 1;
        } else {
            non_ascii += 1;
        }
    }
    let count = ascii.saturating_add(3) / 4 + non_ascii;
    TokenCount {
        count,
        measurement: TokenMeasurement {
            kind: TokenMeasurementKind::Estimated,
            name: "estimate".into(),
            label: "Estimated tokens".into(),
            method: "character-composition heuristic".into(),
            note: "Rough estimate based on ASCII and non-ASCII character counts. It is not an exact tokenizer count; use an exact encoding for billing decisions.".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_is_labelled_and_monotonic() {
        let short = count("hello", TokenizerKind::Estimate);
        let long = count(
            "hello world, this is a longer line",
            TokenizerKind::Estimate,
        );
        assert_eq!(short.measurement.kind, TokenMeasurementKind::Estimated);
        assert!(long.count >= short.count);
    }

    #[test]
    fn exact_counts_are_available_for_known_encodings() {
        let o = count("hello world", TokenizerKind::O200kBase);
        let c = count("hello world", TokenizerKind::Cl100kBase);
        assert_eq!(o.measurement.kind, TokenMeasurementKind::Exact);
        assert_eq!(c.measurement.kind, TokenMeasurementKind::Exact);
        assert!(o.count > 0);
        assert!(c.count > 0);
    }

    #[test]
    fn empty_text_counts_as_zero() {
        let count = count("", TokenizerKind::O200kBase);
        assert_eq!(count.count, 0);
    }
}
