use thiserror::Error;

/// Structured errors returned by the compression engine.
#[derive(Debug, Error)]
pub enum Error {
    #[error("input is {actual} bytes, exceeding the {limit}-byte safety limit")]
    InputTooLarge { actual: usize, limit: usize },

    #[error("forced JSON input is not valid JSON: {message}")]
    InvalidJson { message: String },

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("input could not be classified safely: {0}")]
    Classification(String),

    #[error("internal safety check failed: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;
