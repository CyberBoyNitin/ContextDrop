use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("no input selected: pass a file path or --stdin")]
    NoInput,

    #[error("cannot read {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("cannot read standard input: {0}")]
    ReadStdin(#[source] io::Error),

    #[error("{label} is not valid UTF-8; ContextDrop only processes text")]
    InvalidUtf8 { label: String },

    #[error("{label} is {actual} bytes, above the {limit}-byte limit")]
    TooLarge {
        label: String,
        actual: usize,
        limit: usize,
    },

    #[error("input too large for the configured {limit}-byte limit")]
    TooLargeStream { limit: usize },

    #[error("cannot write {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("cannot write standard output: {0}")]
    WriteStdout(#[source] io::Error),

    #[error("refusing to overwrite existing output file {0}; pass --force to allow it")]
    OutputExists(PathBuf),

    #[error("refusing to overwrite the input file without explicit --force")]
    InputOverwrite(PathBuf),

    #[error("--output must point to a file, not a directory: {0}")]
    OutputIsDirectory(PathBuf),

    #[error("a benchmark path is a directory but has no readable fixture files: {0}")]
    EmptyDirectory(PathBuf),

    #[error("cannot inspect {path}: {source}")]
    InspectFile {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("invalid fixture manifest {path}: {source}")]
    ManifestJson {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid invocation: {0}")]
    Invocation(String),

    #[error(transparent)]
    Engine(#[from] contextdrop_core::Error),

    #[error("cannot serialize JSON output: {0}")]
    Json(#[from] serde_json::Error),

    #[error("benchmark completed, but required evidence was not preserved")]
    BenchmarkEvidenceFailed,
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::TooLarge { .. } | Self::TooLargeStream { .. } => 4,
            Self::BenchmarkEvidenceFailed => 3,
            Self::Engine(contextdrop_core::Error::InputTooLarge { .. }) => 4,
            Self::Engine(contextdrop_core::Error::InvalidJson { .. })
            | Self::Engine(contextdrop_core::Error::InvalidConfig(_))
            | Self::Engine(contextdrop_core::Error::Classification(_))
            | Self::Engine(contextdrop_core::Error::Internal(_)) => 5,
            _ => 1,
        }
    }
}

pub fn is_broken_pipe(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::BrokenPipe
}
