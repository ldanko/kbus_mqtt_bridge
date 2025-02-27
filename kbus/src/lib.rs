//! # kbus Library
//!
//! The **kbus** crate provides a high-level Rust API for interfacing with the K-Bus on
//! WAGO devices. It is built on top of the low-level FFI bindings found in the `kbus-sys`
//! crate, which wrap the WAGO Device Abstraction Layer (DAL).
//!
//! The main entry point to interact with the bus is the [`KBus`] type. For error handling,
//! refer to the [`Error`] type.
use kbus_sys as ffi;

mod dal;
mod error;
mod kbus;

pub use error::Error;
pub use kbus::KBus;
