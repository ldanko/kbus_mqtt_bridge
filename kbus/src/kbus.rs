//! # High-Level K-Bus API
//!
//! This module provides a higher-level abstraction over the DAL interface for interacting
//! with the K-Bus. It defines the main [`KBus`] type as well as helper types for reading and
//! writing process data.

use crate::{
    dal::{ApplicationDeviceInterface, ApplicationState, DeviceId},
    error::{DalResult, Error, Result},
};

/// A writer handle for process data.
///
/// When instantiated, it starts the write operation and commits the data when dropped.
pub struct Writer<'a> {
    dev: &'a mut KBus,
    task_id: u32,
}

impl<'a> Writer<'a> {
    /// Creates a new writer and initiates the write sequence.
    fn new(dev: &'a mut KBus, task_id: u32) -> Result<Writer<'a>> {
        dev.adi.write_start(dev.id, task_id)?;

        Ok(Writer { dev, task_id })
    }

    /// Writes a single bit at the specified offset.
    pub fn write_bit(&mut self, bit_offset: u32, data: &mut u8) -> Result<()> {
        self.dev
            .adi
            .write_bit(self.dev.id, self.task_id, bit_offset, data)
    }

    /// Writes a boolean value at the specified offset.
    pub fn write_bool(&mut self, bit_offset: u32, value: bool) -> Result<()> {
        self.dev
            .adi
            .write_bool(self.dev.id, self.task_id, bit_offset, value)
    }

    /// Writes a series of bytes starting at the given offset.
    pub fn write_bytes(&mut self, offset: u32, data: &mut [u8]) -> Result<()> {
        self.dev
            .adi
            .write_bytes(self.dev.id, self.task_id, offset, data)
    }
}

impl<'a> Drop for Writer<'a> {
    fn drop(&mut self) {
        let _ = self.dev.adi.write_end(self.dev.id, self.task_id);
    }
}

/// A reader handle for process data.
///
/// When instantiated, it begins the read operation and finalizes it upon dropping.
pub struct Reader<'a> {
    dev: &'a mut KBus,
    task_id: u32,
}

impl<'a> Reader<'a> {
    /// Creates a new reader and initiates the read sequence.
    fn new(dev: &'a mut KBus, task_id: u32) -> Result<Reader<'a>> {
        dev.adi.read_start(dev.id, task_id)?;

        Ok(Reader { dev, task_id })
    }

    /// Reads a single bit from the specified offset.
    pub fn read_bit(&mut self, bit_offset: u32, data: &mut u8) -> Result<()> {
        self.dev
            .adi
            .read_bit(self.dev.id, self.task_id, bit_offset, data)
    }

    /// Reads a boolean value from the specified offset.
    pub fn read_bool(&mut self, bit_offset: u32, value: &mut bool) -> Result<()> {
        self.dev
            .adi
            .read_bool(self.dev.id, self.task_id, bit_offset, value)
    }

    /// Reads a series of bytes starting at the given offset.
    pub fn read_bytes(&mut self, offset: u32, data: &mut [u8]) -> Result<()> {
        self.dev
            .adi
            .read_bytes(self.dev.id, self.task_id, offset, data)
    }
}

impl<'a> Drop for Reader<'a> {
    fn drop(&mut self) {
        let _ = self.dev.adi.read_end(self.dev.id, self.task_id);
    }
}

pub struct KBus {
    adi: ApplicationDeviceInterface,
    id: DeviceId,
}

/// The primary type representing a connection to a K-Bus device.
///
/// The [`KBus`] type abstracts the low-level DAL functions and provides methods
/// to control the device, trigger bus cycles, and perform I/O operations.
impl KBus {
    /// Creates a new instance of [`KBus`] by scanning for a device named `"libpackbus"`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DeviceNotFound`] if no matching device is found.
    pub fn new() -> Result<KBus> {
        let mut adi = ApplicationDeviceInterface::new()?;
        for device in adi.get_device_list()? {
            if device.name() == "libpackbus" {
                adi.open_device(device.id())?;
                return Ok(KBus {
                    adi,
                    id: device.id(),
                });
            }
        }
        Err(Error::DeviceNotFound)
    }

    /// Sets the application state to "Running".
    pub fn start(&mut self) -> Result<()> {
        self.adi
            .application_state_changed(ApplicationState::Running)
    }

    /// Sets the application state to "Stopped".
    pub fn stop(&mut self) -> Result<()> {
        self.adi
            .application_state_changed(ApplicationState::Stopped)
    }

    /// Sets the application state to "Unconfigured".
    pub fn reset(&mut self) -> Result<()> {
        self.adi
            .application_state_changed(ApplicationState::Unconfigured)
    }

    /// Triggers a single K-Bus cycle by invoking the device-specific function
    /// `"libpackbus_Push"`.
    pub fn trigger_bus_cycle(&mut self) -> Result<()> {
        // Use function "libpackbus_Push" to trigger one KBUS cycle.
        let retval: i32 = unsafe {
            self.adi
                .call_device_specific_function_simple("libpackbus_Push")?
        };
        match DalResult::from(retval) {
            DalResult::Success => Ok(()),
            DalResult::Failure => Err(Error::DalError),
            DalResult::NotUsed => Err(Error::Unimplemented),
        }
    }

    /// Retrieves the sizes of the device's input and output areas.
    ///
    /// **Note:** The values obtained may require further validation.
    pub fn io_sizes(&mut self) -> Result<(u32, u32)> {
        // TODO: This seems to be broken - returns (12000, 12000),
        //       use API from kbusdemo/getkabusinfo.c to get proper IO size.
        //
        self.adi.get_io_sizes(self.id)
    }

    /// Creates a new [`Writer`] handle to begin a process data write operation.
    pub fn writer(&mut self) -> Result<Writer> {
        // As doc says, task_id is currently unused.
        let task_id = 0;
        Writer::new(self, task_id)
    }

    /// Creates a new [`Reader`] handle to begin a process data read operation.
    pub fn reader(&mut self) -> Result<Reader> {
        // As doc says, task_id is currently unused.
        let task_id = 0;
        Reader::new(self, task_id)
    }
}

impl Drop for KBus {
    fn drop(&mut self) {
        let _ = self.adi.close_device(self.id);
    }
}
