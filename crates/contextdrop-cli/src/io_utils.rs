use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::args::InputSelection;
use crate::error::{is_broken_pipe, CliError};

pub struct LoadedInput {
    pub text: String,
    pub label: String,
}

pub fn read_input(selection: &InputSelection, max_bytes: usize) -> Result<LoadedInput, CliError> {
    match (&selection.file, selection.stdin) {
        (Some(_), true) => Err(CliError::Invocation(
            "a file path and --stdin cannot be used together".into(),
        )),
        (Some(path), false) => {
            let text = read_file_limited(path, max_bytes)?;
            Ok(LoadedInput {
                text,
                label: path.display().to_string(),
            })
        }
        (None, true) => {
            let text = read_stdin_limited(max_bytes)?;
            Ok(LoadedInput {
                text,
                label: "standard input".into(),
            })
        }
        (None, false) => Err(CliError::NoInput),
    }
}

pub fn read_file_limited(path: &Path, max_bytes: usize) -> Result<String, CliError> {
    let label = path.display().to_string();
    let metadata = fs::metadata(path).map_err(|source| CliError::ReadFile {
        path: label.clone(),
        source,
    })?;
    if metadata.is_dir() {
        return Err(CliError::Invocation(format!(
            "input path is a directory, not a file: {label}"
        )));
    }
    if metadata.len() > max_bytes as u64 {
        return Err(CliError::TooLarge {
            label,
            actual: metadata.len().min(usize::MAX as u64) as usize,
            limit: max_bytes,
        });
    }

    let file = fs::File::open(path).map_err(|source| CliError::ReadFile {
        path: label.clone(),
        source,
    })?;
    let bytes = read_limited(file, max_bytes, "input file")?;
    decode_utf8(bytes, &label)
}

pub fn read_stdin_limited(max_bytes: usize) -> Result<String, CliError> {
    let stdin = io::stdin();
    let lock = stdin.lock();
    let bytes = read_limited(lock, max_bytes, "standard input")?;
    decode_utf8(bytes, "standard input")
}

fn read_limited<R: Read>(reader: R, max_bytes: usize, label: &str) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    let limit = max_bytes.saturating_add(1) as u64;
    reader
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|source| {
            if label == "standard input" {
                CliError::ReadStdin(source)
            } else {
                CliError::ReadFile {
                    path: label.into(),
                    source,
                }
            }
        })?;
    if bytes.len() > max_bytes {
        return Err(CliError::TooLargeStream { limit: max_bytes });
    }
    Ok(bytes)
}

fn decode_utf8(bytes: Vec<u8>, label: &str) -> Result<String, CliError> {
    String::from_utf8(bytes).map_err(|_| CliError::InvalidUtf8 {
        label: label.into(),
    })
}

pub fn ensure_output_safe(
    output: &Path,
    input: Option<&Path>,
    force: bool,
) -> Result<(), CliError> {
    if output.is_dir() {
        return Err(CliError::OutputIsDirectory(output.to_path_buf()));
    }
    let same_as_input = input.is_some_and(|input| paths_refer_to_same_file(input, output));
    if same_as_input && !force {
        return Err(CliError::InputOverwrite(output.to_path_buf()));
    }
    if output.exists() && !force {
        return Err(CliError::OutputExists(output.to_path_buf()));
    }
    Ok(())
}

fn paths_refer_to_same_file(first: &Path, second: &Path) -> bool {
    if first == second {
        return true;
    }
    let first = normalize_path(first);
    let second = normalize_path(second);
    match (first, second) {
        (Some(first), Some(second)) => {
            #[cfg(windows)]
            {
                first
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&second.to_string_lossy())
            }
            #[cfg(not(windows))]
            {
                first == second
            }
        }
        _ => false,
    }
}

fn normalize_path(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path).ok();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).ok()?;
    let name = path.file_name()?;
    Some(parent.join(name))
}

pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    fs::write(path, bytes).map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

pub fn write_stdout(bytes: &[u8]) -> Result<(), CliError> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    if let Err(error) = lock.write_all(bytes) {
        if is_broken_pipe(&error) {
            return Ok(());
        }
        return Err(CliError::WriteStdout(error));
    }
    if let Err(error) = lock.flush() {
        if is_broken_pipe(&error) {
            return Ok(());
        }
        return Err(CliError::WriteStdout(error));
    }
    Ok(())
}

pub fn stderr_line(message: &str) {
    let _ = writeln!(io::stderr().lock(), "{message}");
}

/// Expand benchmark path arguments, treating directories as a shallow
/// collection of common text fixtures. No recursive traversal is performed.
pub fn expand_benchmark_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, CliError> {
    let mut result = BTreeSet::new();
    for path in paths {
        let metadata = fs::metadata(path).map_err(|source| CliError::InspectFile {
            path: path.display().to_string(),
            source,
        })?;
        if metadata.is_dir() {
            let entries = fs::read_dir(path).map_err(|source| CliError::InspectFile {
                path: path.display().to_string(),
                source,
            })?;
            let mut added = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|source| CliError::InspectFile {
                    path: path.display().to_string(),
                    source,
                })?;
                let entry_path = entry.path();
                let is_manifest = entry_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("manifest.json"));
                if !is_manifest
                    && entry_path.is_file()
                    && entry_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| {
                            matches!(
                                ext.to_ascii_lowercase().as_str(),
                                "log"
                                    | "txt"
                                    | "json"
                                    | "jsonl"
                                    | "ndjson"
                                    | "out"
                                    | "trace"
                                    | "md"
                            )
                        })
                {
                    added.push(entry_path);
                }
            }
            added.sort();
            if added.is_empty() {
                return Err(CliError::EmptyDirectory(path.clone()));
            }
            result.extend(added);
        } else {
            result.insert(path.clone());
        }
    }
    Ok(result.into_iter().collect())
}
