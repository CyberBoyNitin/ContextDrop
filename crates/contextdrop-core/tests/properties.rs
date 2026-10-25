use contextdrop_core::{compress, CompressionConfig, InputFormat, Strategy, TokenizerKind};
use proptest::prelude::*;

fn config(strategy: Strategy) -> CompressionConfig {
    CompressionConfig {
        strategy,
        tokenizer: TokenizerKind::Estimate,
        max_input_bytes: 200_000,
        ..CompressionConfig::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn arbitrary_utf8_never_panics_and_respects_reported_sizes(
        chars in proptest::collection::vec(any::<char>(), 0..2500)
    ) {
        let input: String = chars.into_iter().collect();
        for strategy in Strategy::all() {
            let result = compress(&input, &config(strategy)).expect("compression must not fail for valid UTF-8");
            prop_assert_eq!(result.metrics.original_bytes, input.len());
            prop_assert_eq!(result.metrics.output_bytes, result.output.len());
            prop_assert_eq!(result.metrics.original_chars, input.chars().count());
            prop_assert_eq!(result.metrics.output_chars, result.output.chars().count());
            prop_assert!(result.preservation.passed, "preservation report must pass after fallback handling");
            prop_assert!(result.metrics.bytes_saved >= 0);
            prop_assert!(result.metrics.byte_reduction_percent.is_finite());
            prop_assert!(result.metrics.token_reduction_percent.is_finite());
        }
    }

    #[test]
    fn protected_user_evidence_survives_all_strategies(
        prefix in proptest::collection::vec("[a-z0-9 ]{0,40}", 0..30),
        suffix in proptest::collection::vec("[a-z0-9 ]{0,40}", 0..30),
    ) {
        let needle = "UNIQUE-EVIDENCE-75f3";
        let mut input = prefix.join("\n");
        input.push('\n');
        input.push_str(needle);
        input.push('\n');
        input.push_str(&suffix.join("\n"));

        for strategy in Strategy::all() {
            let mut cfg = config(strategy);
            cfg.preserve_patterns = vec![needle.to_string()];
            let result = compress(&input, &cfg).unwrap();
            prop_assert!(result.output.contains(needle));
            prop_assert!(result.preservation.passed);
        }
    }

    #[test]
    fn valid_json_outputs_stay_valid(
        records in proptest::collection::vec(
            ("[a-z]{1,12}", any::<u32>(), any::<bool>()),
            0..80
        ),
    ) {
        let array: Vec<serde_json::Value> = records
            .into_iter()
            .map(|(name, count, active)| serde_json::json!({"name": name, "count": count, "active": active}))
            .collect();
        let input = serde_json::to_string_pretty(&array).unwrap();
        let mut cfg = config(Strategy::Balanced);
        cfg.input_format = InputFormat::Json;
        let result = compress(&input, &cfg).unwrap();
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&result.output);
        prop_assert!(parsed.is_ok(), "JSON output must remain valid");
        prop_assert!(result.preservation.passed);
    }
}
