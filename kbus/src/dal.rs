//! # DAL Wrapper Module
//!
//! This module wraps the raw DAL interface provided by `kbus-sys`
//! in a more ergonomic Rust API. It handles initialization, device scanning,
//! and basic I/O operations with the underlying hardware.
//!
//! The module also defines types for representing devices and application state.

use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_void},
    mem,
};

use kbus_sys as ffi;

use crate::error::{DalResult, Error, Result};

const MAX_DAL_DEVICES_COUNT: usize = 10;

/// A helper macro that calls a DAL method and converts its return code into a [`Result<()>`].
///
/// The macro expects that the method returns an integer which can be interpreted
/// using [`DalResult`].
macro_rules! dal_method {
    ($obj: ident . $method: ident ($($args: expr),*)) => {
        match unsafe { (*$obj.ptr).$method.unwrap()($($args),*) }.into() {
            DalResult::Success => Ok(()),
            DalResult::Failure => Err(Error::DalError),
            DalResult::NotUsed => Err(Error::Unimplemented),
        }
    };
}

/// A simple wrapper for a device identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DeviceId(ffi::tDeviceId);

/// Contains the basic information for a device discovered by the DAL.
#[derive(Debug)]
pub struct DeviceInfo {
    id: DeviceId,
    name: String,
}

impl DeviceInfo {
    /// Returns the unique device identifier.
    pub fn id(&self) -> DeviceId {
        self.id
    }

    /// Returns the device name as a string slice.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Represents the state of the PLC application.
#[repr(u32)]
pub enum ApplicationState {
    /// The application is actively running.
    Running = ffi::enApplicationState_ApplicationState_Running,
    /// The application has been stopped.
    Stopped = ffi::enApplicationState_ApplicationState_Stopped,
    /// The application is in an unconfigured state.
    Unconfigured = ffi::enApplicationState_ApplicationState_Unconfigured,
}

/// A safe wrapper around the DAL application interface.
///
/// This type manages the lifecycle of the DAL, including initialization,
/// device scanning, and device management. It also caches devices by name
/// for easier lookup.
pub struct ApplicationDeviceInterface {
    ptr: *mut ffi::tApplicationDeviceInterface,

    devices_by_name: HashMap<String, DeviceId>,
}

// SAFETY: The pointer is only ever dereferenced in methods of this type,
// and we ensure that the device API is initialized correctly.
// The ApplicationDeviceInterface is thread-local and not shared between threads.
unsafe impl Send for ApplicationDeviceInterface {}

impl ApplicationDeviceInterface {
    /// Creates a new DAL interface instance, initializes it,
    /// scans for available devices, and builds an internal device map.
    ///
    /// # Errors
    ///
    /// Returns an error if the initialization or scanning fails.
    pub(super) fn new() -> Result<ApplicationDeviceInterface> {
        let ptr = unsafe {
            let ptr = ffi::adi_GetApplicationInterface();

            assert!(!ptr.is_null());
            assert!((*ptr).Init.is_some());
            assert!((*ptr).Exit.is_some());
            assert!((*ptr).ScanDevices.is_some());
            assert!((*ptr).GetDeviceList.is_some());
            assert!((*ptr).OpenDevice.is_some());
            assert!((*ptr).CloseDevice.is_some());
            assert!((*ptr).WriteStart.is_some());
            assert!((*ptr).WriteBit.is_some());
            assert!((*ptr).WriteBool.is_some());
            assert!((*ptr).WriteBytes.is_some());
            assert!((*ptr).WriteEnd.is_some());
            assert!((*ptr).ReadStart.is_some());
            assert!((*ptr).ReadBit.is_some());
            assert!((*ptr).ReadBool.is_some());
            assert!((*ptr).ReadBytes.is_some());
            assert!((*ptr).ReadEnd.is_some());
            assert!((*ptr).ApplicationStateChanged.is_some());
            assert!((*ptr).CallDeviceSpecificFunction.is_some());

            ptr
        };

        let mut adi = ApplicationDeviceInterface {
            ptr,
            devices_by_name: HashMap::with_capacity(MAX_DAL_DEVICES_COUNT),
        };
        adi.init()?;
        adi.scan_devices()?;
        for device in adi.get_device_list()? {
            adi.devices_by_name
                .insert(device.name().into(), device.id());
        }
        Ok(adi)
    }

    /// Initializes the DAL. Fails in case another instance is already running.
    fn init(&mut self) -> Result<()> {
        dal_method!(self.Init())
    }

    /// Cleans up and exits the DAL.
    fn exit(&mut self) {
        // Doc says `Exit` never fails. But even if it's not true, can't do
        // anything in this case.
        let _ = dal_method!(self.Exit());
    }

    /// Scans the device library path for available devices.
    ///
    /// Checks all shared objects contained in the device library search path
    /// (default path is /usr/lib/dal) and calls their initialisation
    /// functions.
    ///
    /// After this call to `fn get_device_list()` returns a list of all
    /// found devices along with their error states.  
    fn scan_devices(&mut self) -> Result<()> {
        dal_method!(self.ScanDevices())
    }

    /// Retrieves the list of devices discovered by the DAL.
    pub(super) fn get_device_list(&mut self) -> Result<Vec<DeviceInfo>> {
        let mut device_list = [ffi::tDeviceInfo {
            DeviceId: 0,
            DeviceName: std::ptr::null(),
        }; MAX_DAL_DEVICES_COUNT];
        let mut devices_found = 0usize;

        dal_method!(self.GetDeviceList(
            mem::size_of_val(&device_list) as usize,
            device_list.as_mut_ptr(),
            &mut devices_found
        ))?;
        Ok(device_list
            .iter()
            .take(devices_found as usize)
            .map(|d| {
                assert!(!d.DeviceName.is_null());
                DeviceInfo {
                    id: DeviceId(d.DeviceId),
                    name: unsafe { CStr::from_ptr(d.DeviceName).to_string_lossy().into_owned() },
                }
            })
            .collect())
    }

    /// Opens the specified device.
    pub(super) fn open_device(&mut self, device_id: DeviceId) -> Result<()> {
        dal_method!(self.OpenDevice(device_id.0))
    }

    /// Closes the specified device.
    pub(super) fn close_device(&mut self, device_id: DeviceId) -> Result<()> {
        dal_method!(self.CloseDevice(device_id.0))
    }

    /// Retrieves the I/O sizes for a device.
    ///
    /// # Returns
    ///
    /// A tuple `(input_size, output_size)`.
    pub(super) fn get_io_sizes(&mut self, device_id: DeviceId) -> Result<(u32, u32)> {
        let mut input_size = 0;
        let mut output_size = 0;

        dal_method!(self.GetIoSizes(device_id.0, &mut input_size, &mut output_size))?;
        Ok((input_size, output_size))
    }

    /// Starts the process data write operation.
    pub(super) fn write_start(&mut self, device_id: DeviceId, task_id: u32) -> Result<()> {
        dal_method!(self.WriteStart(device_id.0, task_id))
    }

    /// Writes a single bit at the given offset.
    pub(super) fn write_bit(
        &mut self,
        device_id: DeviceId,
        task_id: u32,
        bit_offset: u32,
        data: &mut u8,
    ) -> Result<()> {
        dal_method!(self.WriteBit(device_id.0, task_id, bit_offset, data))
    }

    /// Writes a boolean value at the given offset.
    pub(super) fn write_bool(
        &mut self,
        device_id: DeviceId,
        task_id: u32,
        bit_offset: u32,
        value: bool,
    ) -> Result<()> {
        dal_method!(self.WriteBool(device_id.0, task_id, bit_offset, value))
    }

    /// Writes a sequence of bytes starting at the specified offset.
    pub(super) fn write_bytes(
        &mut self,
        device_id: DeviceId,
        task_id: u32,
        offset: u32,
        data: &mut [u8],
    ) -> Result<()> {
        dal_method!(self.WriteBytes(
            device_id.0,
            task_id,
            offset,
            data.len() as u32,
            data.as_mut_ptr()
        ))
    }

    /// Ends the process data write operation.
    pub(super) fn write_end(&mut self, id: DeviceId, task_id: u32) -> Result<()> {
        dal_method!(self.WriteEnd(id.0, task_id))
    }

    /// Starts the process data read operation.
    pub(super) fn read_start(&mut self, device_id: DeviceId, task_id: u32) -> Result<()> {
        dal_method!(self.ReadStart(device_id.0, task_id))
    }

    /// Reads a single bit from the specified offset.
    pub(super) fn read_bit(
        &mut self,
        device_id: DeviceId,
        task_id: u32,
        bit_offset: u32,
        data: &mut u8,
    ) -> Result<()> {
        dal_method!(self.ReadBit(device_id.0, task_id, bit_offset, data))
    }

    /// Reads a boolean value from the specified offset.
    pub(super) fn read_bool(
        &mut self,
        device_id: DeviceId,
        task_id: u32,
        bit_offset: u32,
        value: &mut bool,
    ) -> Result<()> {
        dal_method!(self.ReadBool(device_id.0, task_id, bit_offset, value))
    }

    /// Reads a sequence of bytes from the device starting at the specified offset.
    pub(super) fn read_bytes(
        &mut self,
        device_id: DeviceId,
        task_id: u32,
        offset: u32,
        data: &mut [u8],
    ) -> Result<()> {
        dal_method!(self.ReadBytes(
            device_id.0,
            task_id,
            offset,
            data.len() as u32,
            data.as_mut_ptr()
        ))
    }

    /// Ends the process data read operation.
    pub(super) fn read_end(&mut self, device_id: DeviceId, task_id: u32) -> Result<()> {
        dal_method!(self.ReadEnd(device_id.0, task_id))
    }

    /// Sets the application state.
    ///
    /// The device is expected to react to the new state accordingly.
    ///
    /// Fails when at least one device returned an error.
    pub(super) fn application_state_changed(&mut self, event: ApplicationState) -> Result<()> {
        let event = ffi::tApplicationStateChangedEvent {
            State: event as u32,
        };
        dal_method!(self.ApplicationStateChanged(event))
    }

    /// Invokes a device-specific function by its name.
    ///
    /// # Arguments
    ///
    /// * `fn_name` - Name of the function.
    /// * `ret_val` - A pointer to store the result.
    ///
    /// # Note
    ///
    /// Additional arguments support is not implemented.
    pub(super) unsafe fn call_device_specific_function(
        &mut self,
        fn_name: &str,
        ret_val: *mut c_void,
    ) -> Result<()> {
        let fn_name = CString::new(fn_name.to_string())?;
        dal_method!(self.CallDeviceSpecificFunction(fn_name.as_ptr(), ret_val))
    }

    /// Invokes a device-specific function and returns a simple value.
    pub(super) unsafe fn call_device_specific_function_simple<T: Copy>(
        &mut self,
        fn_name: &str,
    ) -> Result<T> {
        let mut ret_val: mem::MaybeUninit<T> = mem::MaybeUninit::uninit();
        unsafe {
            self.call_device_specific_function(fn_name, ret_val.as_mut_ptr() as *mut c_void)?;
            Ok(ret_val.assume_init())
        }
    }
}

impl Drop for ApplicationDeviceInterface {
    fn drop(&mut self) {
        self.exit();
    }
}
