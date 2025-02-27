# kbus-sys

kbus-sys is a Rust crate providing low-level FFI bindings to the WAGO Device
Abstraction Layer (DAL) API used for accessing the K-Bus on WAGO devices.
The bindings were automatically generated using
[bindgen](https://github.com/rust-lang/rust-bindgen) based on the original
C header files and are now pregenerated in this crate.

**Note:** Access to the DAL library is necessary.

## Features

Low-level access to the DAL API functions including initialization,
device scanning, opening/closing devices, and process data I/O.

## Requirements

- Linux operating system (targeting WAGO devices, e.g., PFC controllers).
- Access to the DAL library at both build time (for linking) and runtime.
- Rust (latest stable version recommended).

## Building

This crate uses pregenerated bindings, so you don't need the DAL headers at build time.
However, you do need the DAL library and other required libraries at build time for linking as well as at runtime for execution.

For detailed instructions on building Rust applications for WAGO devices,
refer to:

- [WAGO PFC Firmware SDK](https://github.com/WAGO/pfc-firmware-sdk-G2)
- [WAGO PFC How-To Guides](https://github.com/WAGO/pfc-howtos)
  - [HowTo Build Rust Executables](https://github.com/WAGO/pfc-howtos/tree/master/HowTo_BuildRustExecutables)

## License

This crate is licensed under the MIT License. Users must also comply with the
licenses of the original WAGO DAL headers and library.

## Contributing

Contributions and bug reports are welcome. Please open an issue or submit a
pull request on the GitHub repository.

## Disclaimer

kbus-sys provides only the raw FFI bindings. It is the user's responsibility
to build safe abstractions on top of these bindings and to comply with all
licensing requirements of the original WAGO DAL.
