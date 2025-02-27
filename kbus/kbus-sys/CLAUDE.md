# KBUS-SYS Development Guide

## Build Commands
- Build: `cargo build`
- Release build: `cargo build --release` 
- Test: `cargo test`
- Single test: `cargo test test_name`
- Documentation: `cargo doc --open`
- Lint: `cargo clippy`
- Format: `cargo fmt`

## Environment Setup
- Required env var: `PTXPROJ_PATH` must point to WAGO PFC Firmware SDK path

## Code Style Guidelines
- Use Rust 2024 edition features
- Follow standard Rust naming conventions:
  - snake_case for functions, variables, files
  - CamelCase for types and traits
  - SCREAMING_SNAKE_CASE for constants
- Explicitly handle errors with Result types
- Minimize unsafe blocks, document when necessary
- Group imports by external crates first, then standard library
- Use explicit type annotations for public API functions
- Document public API with rustdoc (///) comments
- Use consistent 4-space indentation

This is a low-level FFI crate - follow bindgen-generated style for FFI interfaces.