# kbus

## Overview

`kbus` is a Rust library that provides a high-level interface for interacting
with the K-Bus on WAGO devices. It is built on top of `kbus-sys`, which provides
low-level FFI bindings to the WAGO Device Abstraction Layer (DAL). The library
allows for reading and writing process data, managing device states, and
triggering bus cycles.

## Features

- Safe Rust wrapper around the WAGO DAL.
- High-level API for K-Bus interaction.
- Support for reading and writing process data.

## Requirements

To use this library, you need WAGO PFC firmware SDK and associated libraries for both build time (linking) and runtime use.
The bindings are pregenerated, so you don't need to regenerate them or have the headers
available during build time, but the DAL libraries themselves are required during compilation for linking.

For reference, the source repositories are:

- [WAGO PFC Firmware SDK](https://github.com/WAGO/pfc-firmware-sdk-G2)
- [WAGO PFC HowTos](https://github.com/WAGO/pfc-howtos)
  - [Building Rust Executables](https://github.com/WAGO/pfc-howtos/tree/master/HowTo_BuildRustExecutables)

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE)
file for details.

## Disclaimer

This library interacts with proprietary WAGO systems. Ensure compliance with
all relevant licensing and usage policies when using it in your projects.
