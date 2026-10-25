use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use contextdrop_core::{
    run_benchmark, BenchmarkCase, BenchmarkRun, FixtureManifest, FixtureSpec, InputFormat,
};
use serde::Serialize;

use crate::args::EngineArgs;
use crate::error::CliError;
use crate::io_utils::{expand_benchmark_paths, read_file_limited};

const MANIFEST_MAX_BYTES: usize = 1_000_000;

#[derive(Debug, Serialize)]
pub struct CliBenchmarkReport {
    pub report_version: u32,
    pub engine_version: String,
    pub tokenizer: String,
    pub files: Vec<CliBenchmarkFile>,
}

#[derive(Debug, Serialize)]
pub struct CliBenchmarkFile {
    /// Local display path. The CLI never uploads this value.
    pub path: String,
    pub run: BenchmarkRun,
}

impl CliBenchmarkReport {
    pub fn all_checks_passed(&self) -> bool {
        self.files.iter().all(|file| {
            file.run.all_critical_checks_passed()
                && file
                    .run
                    .results
                    .iter()
                    .all(|result| result.compression.preservation.passed)
        })
    }

    pub fn evidence_totals(&self) -> (usize, usize) {
        let mut passed = 0;
        let mut total = 0;
        for file in &self.files {
            for result in &file.run.results {
                for check in &result.critical_checks {
                    total += 1;
                    if check.passed {
                        passed += 1;
                    }
                }
            }
        }
        (passed, total)
    }
}

pub fn execute_benchmark(
    raw_paths: &[PathBuf],
    explicit_manifest: Option<&Path>,
    engine: &EngineArgs,
) -> Result<CliBenchmarkReport, CliError> {
    let paths = expand_benchmark_paths(raw_paths)?;
    let loaded_manifest = load_manifest(explicit_manifest, &paths)?;
    let manifest_map = loaded_manifest
        .as_ref()
        .map(|(base, manifest)| fixture_map(base, manifest))
        .unwrap_or_default();

    let mut base_config = engine.to_config(contextdrop_core::Strategy::Balanced);
    // Benchmarking always runs all strategies. Per-fixture formats are applied
    // by the core runner.
    base_config.strategy = contextdrop_core::Strategy::Balanced;

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let text = read_file_limited(&path, engine.max_input_bytes)?;
        let spec = manifest_map.get(&path_key(&path)).cloned();
        let mut format = InputFormat::from(engine.input_format);
        let mut name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fixture")
            .to_string();
        let mut case = if let Some(spec) = &spec {
            if format == InputFormat::Auto {
                format = spec.format;
            }
            name = spec.name.clone();
            BenchmarkCase::from_fixture(spec, text)
        } else {
            BenchmarkCase::user(name, text, format)
        };
        case.format = format;

        let run = run_benchmark(&case, &base_config)?;
        files.push(CliBenchmarkFile {
            path: path.display().to_string(),
            run,
        });
    }

    Ok(CliBenchmarkReport {
        report_version: 1,
        engine_version: contextdrop_core::VERSION.into(),
        tokenizer: engine.tokenizer_name().into(),
        files,
    })
}

fn load_manifest(
    explicit: Option<&Path>,
    paths: &[PathBuf],
) -> Result<Option<(PathBuf, FixtureManifest)>, CliError> {
    if let Some(path) = explicit {
        let manifest: FixtureManifest = read_manifest(path)?;
        let base = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        return Ok(Some((base, manifest)));
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("fixtures").join("manifest.json"));
        candidates.push(cwd.join("manifest.json"));
    }
    for path in paths {
        if let Some(parent) = path.parent() {
            candidates.push(parent.join("manifest.json"));
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.join("manifest.json"));
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    for candidate in candidates {
        let key = path_key(&candidate);
        if !seen.insert(key) || !candidate.is_file() {
            continue;
        }
        let manifest: FixtureManifest = read_manifest(&candidate)?;
        let base = candidate
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        return Ok(Some((base, manifest)));
    }
    Ok(None)
}

fn read_manifest(path: &Path) -> Result<FixtureManifest, CliError> {
    let text = read_file_limited(path, MANIFEST_MAX_BYTES)?;
    serde_json::from_str(&text).map_err(|source| CliError::ManifestJson {
        path: path.display().to_string(),
        source,
    })
}

fn fixture_map(base: &Path, manifest: &FixtureManifest) -> BTreeMap<String, FixtureSpec> {
    let mut map = BTreeMap::new();
    for spec in &manifest.fixtures {
        let path = base.join(&spec.file);
        map.insert(path_key(&path), spec.clone());
    }
    map
}

fn path_key(path: &Path) -> String {
    let normalized = fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        }
    });
    let text = normalized.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        text.to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_keys_are_stable_for_existing_files() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let first = path_key(file.path());
        let second = path_key(file.path());
        assert_eq!(first, second);
    }
}
