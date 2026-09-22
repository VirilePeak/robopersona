//! Error types for botpack-core.

use thiserror::Error;

/// All errors produced by botpack-core.
#[derive(Debug, Error)]
pub enum Error {
    /// Manifest failed structural or semantic validation.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    /// Package name failed validation.
    #[error("invalid package name {input:?}: {reason}")]
    InvalidName {
        /// The rejected input.
        input: String,
        /// Why it was rejected.
        reason: &'static str,
    },

    /// JSON (de)serialization failure.
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Convenient result alias.
pub type Result<T> = std::result::Result<T, Error>;
