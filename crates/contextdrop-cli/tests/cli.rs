use assert_cmd::assert::Assert;
use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn command() -> Command {
    Command::cargo_bin("contextdrop").expect("compiled ContextDrop CLI binary")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn parse_stdout(assert: Assert) -> Value {
    serde_json::from_slice(&assert.get_output().stdout).expect("valid JSON on stdout")
}

fn repeated_log() -> String {
    let mut text = String::new();
    for _ in 0..12 {
        text.push_str("waiting for file lock on build/target/artifact-cache.lock\n");
    }
    text.push_str("error: could not compile `demo`\n");
    text.push_str("error[E0308]: mismatched types\n");
    text.push_str("  --> src/main.rs:14:37\n");
    text.push_str("exit code: 101\n");
    text
}

fn non_consecutive_log() -> String {
    let repeated =
        "DEBUG cache lookup completed with no changes for request hash 0123456789abcdef\n";
    let mut text = String::new();
    for index in 0..10 {
        text.push_str(repeated);
        text.push_str(&format!("unique event marker {index}\n"));
    }
    text.push_str("error: final failure must remain visible\n");
    text.push_str("exit code: 1\n");
    text
}

#[test]
fn compress_file_removes_noise_and_keeps_failure_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("build.log");
    let input = repeated_log();
    fs::write(&input_path, &input).unwrap();

    let assert = command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("compress")
        .arg(&input_path)
        .arg("--strategy")
        .arg("balanced")
        .arg("--tokenizer")
        .arg("estimate")
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(output.len() < input.len());
    assert!(output.contains("error[E0308]: mismatched types"));
    assert!(output.contains("src/main.rs:14:37"));
    assert!(output.contains("exit code: 101"));
    assert!(output.contains("repeated lines omitted"));
    assert!(assert.get_output().stderr.is_empty());
}

#[test]
fn stdin_json_reports_real_metrics_and_measurement_kind() {
    let input = repeated_log();
    let assert = command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("compress")
        .arg("--stdin")
        .arg("--format")
        .arg("json")
        .arg("--strategy")
        .arg("balanced")
        .arg("--tokenizer")
        .arg("estimate")
        .write_stdin(input.clone())
        .assert()
        .success();

    let body = parse_stdout(assert);
    assert_eq!(body["strategy"], "balanced");
    assert_eq!(body["fallback_used"], false);
    assert_eq!(body["transformation_applied"], true);
    assert_eq!(body["preservation"]["passed"], true);
    assert_eq!(
        body["metrics"]["original_bytes"].as_u64().unwrap() as usize,
        input.len()
    );
    assert!(
        body["metrics"]["output_bytes"].as_u64().unwrap() < input.len() as u64,
        "output must be smaller"
    );
    assert_eq!(body["metrics"]["token_measurement"]["kind"], "estimated");
    let output = body["output"].as_str().unwrap();
    assert!(output.contains("error[E0308]: mismatched types"));
    assert!(output.contains("exit code: 101"));
    assert!(
        body["omissions"]["repeated_lines_omitted"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn strategies_change_scope_and_preserve_evidence() {
    let input = non_consecutive_log();

    let conservative = parse_stdout(
        command()
            .current_dir(repo_root())
            .arg("--quiet")
            .arg("compress")
            .arg("--stdin")
            .arg("--format")
            .arg("json")
            .arg("--strategy")
            .arg("conservative")
            .arg("--tokenizer")
            .arg("estimate")
            .write_stdin(input.clone())
            .assert()
            .success(),
    );
    let balanced = parse_stdout(
        command()
            .current_dir(repo_root())
            .arg("--quiet")
            .arg("compress")
            .arg("--stdin")
            .arg("--format")
            .arg("json")
            .arg("--strategy")
            .arg("balanced")
            .arg("--tokenizer")
            .arg("estimate")
            .write_stdin(input.clone())
            .assert()
            .success(),
    );
    let aggressive = parse_stdout(
        command()
            .current_dir(repo_root())
            .arg("--quiet")
            .arg("compress")
            .arg("--stdin")
            .arg("--format")
            .arg("json")
            .arg("--strategy")
            .arg("aggressive")
            .arg("--tokenizer")
            .arg("estimate")
            .write_stdin(input.clone())
            .assert()
            .success(),
    );

    assert_eq!(
        conservative["omissions"]["repeated_lines_omitted"], 0,
        "conservative only collapses consecutive runs"
    );
    assert_eq!(
        balanced["omissions"]["repeated_lines_omitted"], 0,
        "balanced does not remove non-consecutive duplicates"
    );
    assert!(
        aggressive["omissions"]["repeated_lines_omitted"]
            .as_u64()
            .unwrap()
            > 0,
        "aggressive must remove non-consecutive duplicates"
    );

    for body in [&balanced, &aggressive] {
        assert_eq!(body["preservation"]["passed"], true);
        let output = body["output"].as_str().unwrap();
        assert!(output.contains("error: final failure must remain visible"));
        assert!(output.contains("exit code: 1"));
    }
}

#[test]
fn literal_preserve_pattern_blocks_removal() {
    let repeated = "ACME-42 customer correlation identifier\n";
    let input = repeated.repeat(12) + "error: keep this too\n";

    let without = parse_stdout(
        command()
            .current_dir(repo_root())
            .arg("--quiet")
            .arg("compress")
            .arg("--stdin")
            .arg("--format")
            .arg("json")
            .arg("--strategy")
            .arg("balanced")
            .arg("--tokenizer")
            .arg("estimate")
            .write_stdin(input.clone())
            .assert()
            .success(),
    );
    let with_pattern = parse_stdout(
        command()
            .current_dir(repo_root())
            .arg("--quiet")
            .arg("compress")
            .arg("--stdin")
            .arg("--format")
            .arg("json")
            .arg("--strategy")
            .arg("balanced")
            .arg("--tokenizer")
            .arg("estimate")
            .arg("--preserve")
            .arg("ACME-42")
            .write_stdin(input)
            .assert()
            .success(),
    );

    assert!(
        without["omissions"]["repeated_lines_omitted"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(with_pattern["omissions"]["repeated_lines_omitted"], 0);
    assert!(with_pattern["output"].as_str().unwrap().contains("ACME-42"));
}

#[test]
fn compare_json_contains_real_unified_diff() {
    let input = repeated_log();
    let assert = command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("compare")
        .arg("--stdin")
        .arg("--format")
        .arg("json")
        .arg("--context")
        .arg("2")
        .arg("--tokenizer")
        .arg("estimate")
        .write_stdin(input.clone())
        .assert()
        .success();

    let body = parse_stdout(assert);
    assert_eq!(body["original"], input);
    let diff = body["unified_diff"].as_str().unwrap();
    assert!(diff.contains("--- original"));
    assert!(diff.contains("+++ contextdrop"));
    assert!(diff.contains("waiting for file lock"));
    assert_eq!(body["compression"]["preservation"]["passed"], true);
}

#[test]
fn output_file_requires_force_before_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let input_path = dir.path().join("input.log");
    let output_path = dir.path().join("output.txt");
    fs::write(&input_path, repeated_log()).unwrap();
    fs::write(&output_path, "do not clobber this file\n").unwrap();

    command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("compress")
        .arg(&input_path)
        .arg("--strategy")
        .arg("balanced")
        .arg("--tokenizer")
        .arg("estimate")
        .arg("--output")
        .arg(&output_path)
        .assert()
        .code(1)
        .stderr(contains("refusing to overwrite existing output file"));

    assert_eq!(
        fs::read_to_string(&output_path).unwrap(),
        "do not clobber this file\n"
    );

    command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("compress")
        .arg(&input_path)
        .arg("--strategy")
        .arg("balanced")
        .arg("--tokenizer")
        .arg("estimate")
        .arg("--output")
        .arg(&output_path)
        .arg("--force")
        .assert()
        .success();

    let written = fs::read_to_string(&output_path).unwrap();
    assert!(written.contains("error[E0308]: mismatched types"));
    assert!(written.contains("exit code: 101"));
}

#[test]
fn oversized_input_uses_documented_exit_code() {
    let input = "x".repeat(500);
    command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("compress")
        .arg("--stdin")
        .arg("--max-input-bytes")
        .arg("100")
        .arg("--tokenizer")
        .arg("estimate")
        .write_stdin(input)
        .assert()
        .code(4)
        .stderr(contains(
            "input too large for the configured 100-byte limit",
        ));
}

#[test]
fn invalid_forced_json_uses_engine_exit_code() {
    command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("compress")
        .arg("--stdin")
        .arg("--input-format")
        .arg("json")
        .arg("--tokenizer")
        .arg("estimate")
        .write_stdin("this is not json\n")
        .assert()
        .code(5)
        .stderr(contains("is not valid JSON"));
}

fn fixture_manifest_path() -> PathBuf {
    repo_root().join("fixtures/manifest.json")
}

fn fixture_path(name: &str) -> PathBuf {
    repo_root().join("fixtures").join(name)
}

#[test]
fn benchmark_fixture_passes_required_evidence() {
    let assert = command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("benchmark")
        .arg(fixture_path("rust-build-failure.log"))
        .arg("--manifest")
        .arg(fixture_manifest_path())
        .arg("--format")
        .arg("json")
        .arg("--tokenizer")
        .arg("estimate")
        .assert()
        .success();

    let body = parse_stdout(assert);
    assert_eq!(body["files"].as_array().unwrap().len(), 1);
    let run = &body["files"][0]["run"];
    assert_eq!(run["case"]["id"], "rust-build-failure");
    assert_eq!(run["results"].as_array().unwrap().len(), 3);
    for result in run["results"].as_array().unwrap() {
        assert_eq!(result["compression"]["preservation"]["passed"], true);
        for check in result["critical_checks"].as_array().unwrap() {
            assert_eq!(check["passed"], true, "evidence check failed: {check}");
        }
    }
}

fn write_missing_evidence_case(dir: &Path) -> (PathBuf, PathBuf) {
    let fixture = dir.join("broken.log");
    fs::write(
        &fixture,
        "starting build\nfinished build without the expected marker\n",
    )
    .unwrap();
    let manifest = dir.join("manifest.json");
    let value = serde_json::json!({
        "version": 1,
        "fixtures": [{
            "id": "missing-evidence",
            "name": "Missing evidence test",
            "description": "A deliberately unsatisfied manifest assertion.",
            "file": "broken.log",
            "category": "Build failure",
            "format": "build-log",
            "must_preserve": ["THIS-MARKER-MUST-APPEAR"],
            "notes": null
        }]
    });
    fs::write(&manifest, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    (fixture, manifest)
}

#[test]
fn benchmark_missing_evidence_uses_exit_code_three() {
    let dir = tempfile::tempdir().unwrap();
    let (fixture, manifest) = write_missing_evidence_case(dir.path());

    command()
        .current_dir(repo_root())
        .arg("--quiet")
        .arg("benchmark")
        .arg(fixture)
        .arg("--manifest")
        .arg(manifest)
        .arg("--format")
        .arg("json")
        .arg("--tokenizer")
        .arg("estimate")
        .assert()
        .code(3)
        .stderr(contains("required evidence was not preserved"));
}
