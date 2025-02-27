//! # Mock K-Bus API
//!
//! This module provides a mock implementation of the K-Bus API for testing.

use std::sync::{Arc, Mutex, LazyLock};

use crate::error::{Error, Result};
use bitvec::prelude::*;

// Shared state for simulating I/O
struct KBusState {
    input_data: BitVec<u8>,
    output_data: BitVec<u8>,
}

impl Default for KBusState {
    fn default() -> Self {
        Self {
            input_data: bitvec![u8, LocalBits; 0; 90],
            output_data: bitvec![u8, LocalBits; 0; 90],
        }
    }
}

// Global state shared between all instances
static KBUS_STATE: LazyLock<Arc<Mutex<KBusState>>> =
    LazyLock::new(|| Arc::new(Mutex::new(KBusState::default())));

/// A writer handle for process data.
pub struct Writer<'a> {
    _dev: &'a mut KBus,
    _task_id: u32,
}

impl<'a> Writer<'a> {
    /// Creates a new writer and initiates the write sequence.
    fn new(_dev: &'a mut KBus, _task_id: u32) -> Result<Writer<'a>> {
        Ok(Writer {
            _dev,
            _task_id,
        })
    }

    /// Writes a single bit at the specified offset.
    pub fn write_bit(&mut self, bit_offset: u32, data: &mut u8) -> Result<()> {
        let mut state = KBUS_STATE.lock().unwrap();
        let bit_offset = bit_offset as usize;

        if bit_offset >= state.output_data.len() {
            return Err(Error::OperationFailed("Offset out of range".to_string()));
        }

        state.output_data.set(bit_offset, *data & 1 != 0);
        Ok(())
    }

    /// Writes a boolean value at the specified offset.
    pub fn write_bool(&mut self, bit_offset: u32, value: bool) -> Result<()> {
        let mut state = KBUS_STATE.lock().unwrap();
        let bit_offset = bit_offset as usize;

        if bit_offset >= state.output_data.len() {
            return Err(Error::OperationFailed("Offset out of range".to_string()));
        }

        state.output_data.set(bit_offset, value);
        Ok(())
    }

    /// Writes a series of bytes starting at the given offset.
    pub fn write_bytes(&mut self, offset: u32, data: &mut [u8]) -> Result<()> {
        let mut state = KBUS_STATE.lock().unwrap();
        let bit_offset = offset as usize;

        // Check if we have enough space (each byte is 8 bits)
        if bit_offset + (data.len() * 8) > state.output_data.len() {
            return Err(Error::OperationFailed(
                "Write exceeds buffer size".to_string(),
            ));
        }

        // Write each byte, bit by bit
        for (i, byte) in data.iter().enumerate() {
            let byte_start = bit_offset + (i * 8);
            for j in 0..8 {
                let bit_value = (*byte >> j) & 1 != 0;
                state.output_data.set(byte_start + j, bit_value);
            }
        }

        Ok(())
    }
}

/// A reader handle for process data.
pub struct Reader<'a> {
    _dev: &'a mut KBus,
    _task_id: u32,
}

impl<'a> Reader<'a> {
    /// Creates a new reader and initiates the read sequence.
    fn new(_dev: &'a mut KBus, _task_id: u32) -> Result<Reader<'a>> {
        Ok(Reader {
            _dev,
            _task_id,
        })
    }

    /// Reads a single bit from the specified offset.
    pub fn read_bit(&mut self, bit_offset: u32, data: &mut u8) -> Result<()> {
        let state = KBUS_STATE.lock().unwrap();
        let bit_offset = bit_offset as usize;

        if bit_offset >= state.input_data.len() {
            return Err(Error::OperationFailed("Offset out of range".to_string()));
        }

        *data = state.input_data[bit_offset] as u8;
        Ok(())
    }

    /// Reads a boolean value from the specified offset.
    pub fn read_bool(&mut self, bit_offset: u32, value: &mut bool) -> Result<()> {
        let state = KBUS_STATE.lock().unwrap();
        let bit_offset = bit_offset as usize;

        if bit_offset >= state.input_data.len() {
            return Err(Error::OperationFailed("Offset out of range".to_string()));
        }

        *value = state.input_data[bit_offset];
        Ok(())
    }

    /// Reads a series of bytes starting at the given offset.
    pub fn read_bytes(&mut self, offset: u32, data: &mut [u8]) -> Result<()> {
        let state = KBUS_STATE.lock().unwrap();
        let bit_offset = offset as usize;

        // Check if we have enough bits (each byte is 8 bits)
        if bit_offset > state.input_data.len() {
            return Err(Error::OperationFailed(
                "Read exceeds buffer size".to_string(),
            ));
        }

        // Read each byte, bit by bit
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = 0; // Clear the byte
            let byte_start = bit_offset + (i * 8);

            for j in 0..8 {
                if byte_start + j >= state.input_data.len() {
                    break;
                }
                if state.input_data[byte_start + j] {
                    *byte |= 1 << j;
                }
            }
        }

        Ok(())
    }
}

/// The primary type representing a mock connection to a K-Bus device.
pub struct KBus {
    is_open: bool,
}

impl KBus {
    /// Creates a new instance of [`KBus`] simulating a device named "libpackbus".
    pub fn new() -> Result<KBus> {
        Ok(KBus { is_open: true })
    }

    /// Sets the application state to "Running".
    pub fn start(&mut self) -> Result<()> {
        Ok(())
    }

    /// Sets the application state to "Stopped".
    pub fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    /// Sets the application state to "Unconfigured".
    pub fn reset(&mut self) -> Result<()> {
        Ok(())
    }

    /// Simulates triggering a K-Bus cycle.
    ///
    /// In this mock implementation, it copies output data to input data
    /// to simulate a loopback behavior.
    pub fn trigger_bus_cycle(&mut self) -> Result<()> {
        // let mut state = KBUS_STATE.lock().unwrap();
        // let output_data = state.output_data.clone();
        // state.input_data.clone_from(&output_data);
        Ok(())
    }

    /// Returns fixed I/O sizes for the mock device.
    pub fn io_sizes(&mut self) -> Result<(u32, u32)> {
        // Return fixed sizes that match your expectations
        Ok((90, 90))
    }

    /// Creates a new [`Writer`] handle to begin a process data write operation.
    pub fn writer(&mut self) -> Result<Writer> {
        let task_id = 0;
        Writer::new(self, task_id)
    }

    /// Creates a new [`Reader`] handle to begin a process data read operation.
    pub fn reader(&mut self) -> Result<Reader> {
        let task_id = 0;
        Reader::new(self, task_id)
    }
}

impl Drop for KBus {
    fn drop(&mut self) {
        self.is_open = false;
    }
}

// Public helper functions for tests

/// Set a simulated input bit value, useful for tests.
pub fn set_input_bit(bit_offset: u32, value: bool) -> Result<()> {
    let mut state = KBUS_STATE.lock().unwrap();
    let bit_offset = bit_offset as usize;

    if bit_offset >= state.input_data.len() {
        return Err(Error::OperationFailed("Offset out of range".to_string()));
    }

    state.input_data.set(bit_offset, value);
    Ok(())
}

/// Get the current state of an output bit, useful for tests.
pub fn get_output_bit(bit_offset: u32) -> Result<bool> {
    let state = KBUS_STATE.lock().unwrap();
    let bit_offset = bit_offset as usize;

    if bit_offset >= state.output_data.len() {
        return Err(Error::OperationFailed("Offset out of range".to_string()));
    }

    Ok(state.output_data[bit_offset])
}

/// Reset all simulated I/O data to default values, useful between tests.
pub fn reset_state() {
    let mut state = KBUS_STATE.lock().unwrap();
    state.input_data.fill(false);
    state.output_data.fill(false);
}
