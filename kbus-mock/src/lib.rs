//! # kbus-mock Library
//!
//! Mock implementation of the kbus crate for testing.

mod error;
mod kbus;

pub use error::Error;
pub use kbus::{KBus, get_output_bit, reset_state, set_input_bit};
