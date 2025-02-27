#!/bin/bash
set -e

if [ -z "$PTXPROJ_PATH" ]; then
  echo "Error: PTXPROJ_PATH environment variable is not set."
  echo "Set it to your WAGO PFC Firmware SDK path, e.g.:"
  echo "export PTXPROJ_PATH=/path/to/pfc-firmware-sdk/ptxproj"
  exit 1
fi

echo "Generating bindings using PTXPROJ_PATH: $PTXPROJ_PATH"

# Ensure bindgen is installed
if ! command -v bindgen &> /dev/null; then
  echo "bindgen not found. Installing..."
  cargo install bindgen-cli
fi

# Run bindgen
bindgen wrapper.h \
  --output src/bindings.rs \
  -- \
  --target="armv7-unknown-linux-gnueabihf" \
  --sysroot="$PTXPROJ_PATH/platform-wago-pfcXXX/sysroot-target" \
  -I"$PTXPROJ_PATH/platform-wago-pfcXXX/sysroot-target/usr/include"

echo "Bindings successfully generated to src/bindings.rs"
