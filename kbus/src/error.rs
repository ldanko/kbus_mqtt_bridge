//! # Error Handling
//!
//! This module defines the error types used across the **kbus** crate.
//! It includes conversion utilities for the raw DAL return values as well as
//! error types for string conversion and device-related issues.

use std::ffi::NulError;

use thiserror::Error;

use crate::ffi;

/// An enumeration of the possible result codes returned by DAL functions.
#[repr(i32)]
pub enum DalResult {
    Success = ffi::DAL_SUCCESS as i32,
    Failure = ffi::DAL_FAILURE,
    NotUsed = ffi::DAL_NOTUSED as i32,
}

impl From<i32> for DalResult {
    fn from(value: i32) -> DalResult {
        match value {
            -1 => DalResult::Failure,
            0 => DalResult::Success,
            1 => DalResult::NotUsed,
            v => {
                eprintln!("unexpected DAL result value: {}", v);
                DalResult::Failure
            }
        }
    }
}

/// Error types returned by the kbus library.
#[derive(Debug, Error)]
pub enum Error {
    /// The function is not implemented by the device.
    #[error("function is not implemented by the device")]
    Unimplemented,
    /// A generic error returned from the DAL.
    #[error("Operation failed")]
    DalError,
    /// An interior nul byte was found in a string.
    #[error("interior nul byte found")]
    NulError,
    /// The specified device was not found.
    #[error("device not found")]
    DeviceNotFound,
}

impl From<NulError> for Error {
    fn from(_: NulError) -> Error {
        Error::NulError
    }
}

/// A convenient type alias for results returned by the kbus library.
pub type Result<T> = std::result::Result<T, Error>;
