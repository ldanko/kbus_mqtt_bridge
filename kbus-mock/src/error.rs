//! # Error Handling
//!
//! Error types for the kbus-mock crate.

use thiserror::Error;

/// Error types returned by the kbus-mock library.
#[derive(Debug, Error)]
pub enum Error {
    /// The function is not implemented.
    #[error("function is not implemented")]
    Unimplemented,
    /// The specified device was not found.
    #[error("device not found")]
    DeviceNotFound,
    /// A generic operation error.
    #[error("operation failed: {0}")]
    OperationFailed(String),
}

/// A convenient type alias for results returned by the kbus-mock library.
pub type Result<T> = std::result::Result<T, Error>;
