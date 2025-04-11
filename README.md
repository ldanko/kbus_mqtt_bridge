# KBUS MQTT Bridge

A bridge application that connects WAGO PFC200's K-Bus I/O modules with MQTT for
industrial and smart home automation systems. This enables bidirectional communication
between PLC digital I/O and MQTT brokers, facilitating IoT integration for both industrial
control systems and smart building automation.

## Features

- Bidirectional communication between WAGO PFC K-Bus I/O and MQTT
- Configurable mapping between K-Bus I/O points and MQTT topics
- Industrial IoT and smart home integration
- Support for digital I/O, sensors, and actuators
- Heartbeat messages for monitoring
- Support for WAGO PFC200 controllers

## Requirements

- WAGO PFC200 controller with firmware supporting K-Bus operations
- WAGO PFC Firmware SDK for cross-compilation
- Rust toolchain 1.85.0 or newer
- ARM GCC toolchain for cross-compilation

## Installation

### Cross-Compilation Setup

1. Set up the development environment:

```bash
# Clone the repository
git clone https://github.com/ldanko/kbus_mqtt_bridge.git
cd kbus_mqtt_bridge

# Set up the development environment
export PTXPROJ_PATH=/path/to/wago/pfc-firmware-sdk-G2/ptxproj
```

2. Configure the ARM GCC toolchain by creating `.cargo/config.toml`

```toml
[target.armv7-unknown-linux-gnueabihf]
linker = "/path/to/arm-linux-gnueabihf-gcc"

[extra-link-arg]
sysroot="/path/to/ptxproj/platform-wago-pfcXXX/sysroot-target"
```

3. Build the application:

```bash
cargo build --target=armv7-unknown-linux-gnueabihf --release
```

## Configuration

The application can be configured using:
1. Environment variables (highest priority)
2. Config file from command line argument 
3. Config file from environment variable
4. Default values (lowest priority)

### Configuration File

The application uses a TOML configuration file. By default, it looks for:
- File path specified as a command-line argument
- File path specified in the `KBUS_BRIDGE_CONFIG_FILE` environment variable

### `config.toml` Example

```toml
# Device name used in MQTT topics
device_name = "pfc200_controller"

# MQTT broker connection settings
[mqtt]
broker_host = "mqtt.example.com"
broker_port = 1883
# Optional username and password for MQTT authentication
# username = "mqtt_user"
# password = "secret_password"
keepalive = "300s"  # Human-readable duration format
heartbeat_interval = "60s"  # Human-readable duration format
```

### Environment Variables

You can override any configuration value using environment variables:

| Environment Variable                  | Description                                       | Default Value      |
|---------------------------------------|---------------------------------------------------|--------------------|
| `KBUS_BRIDGE_DEVICE_NAME`             | Device name for MQTT topics                       | "kbus_mqtt_bridge" |
| `KBUS_BRIDGE_MQTT_HOST`               | MQTT broker hostname or IP address                | "localhost"        |
| `KBUS_BRIDGE_MQTT_PORT`               | MQTT broker port                                  | 1883               |
| `KBUS_BRIDGE_MQTT_USERNAME`           | MQTT username for authentication (optional)       | None               |
| `KBUS_BRIDGE_MQTT_PASSWORD`           | MQTT password for authentication (optional)       | None               |
| `KBUS_BRIDGE_MQTT_KEEPALIVE`          | Connection keepalive in seconds                   | 300 (5 minutes)    |
| `KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL` | Heartbeat interval in seconds (0 to disable)      | 60 (1 minute)      |
| `KBUS_BRIDGE_CONFIG_FILE`             | Path to config file (if not provided as argument) | None               |

### Configuration Validation

The application validates all configuration values:

- Device name: Must not be empty and cannot contain whitespace or MQTT special characters (`/`, `+`, `#`)
- MQTT broker host: Cannot be empty
- MQTT broker port: Cannot be 0
- Keepalive: Must be between 5 seconds and 24 hours
- Heartbeat interval: Must be 0 (disabled) or between 1 second and 1 hour

## Use Case Examples

### Industrial Applications
- Connect factory floor PLCs to cloud-based monitoring systems
- Enable real-time production data collection via MQTT
- Interface legacy industrial equipment with modern IoT platforms
- Implement predictive maintenance using sensor data
- Monitor remote industrial sites through secure MQTT connections

### Smart Home Applications
- Connect physical wall switches to control smart lights
- Bridge heating system controls with Home Assistant or other platforms
- Automate blinds and shades based on time or temperature
- Integrate legacy electrical systems with modern IoT platforms
- Provide reliable, hardware-based automation with MQTT flexibility

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [WAGO PFC Firmware SDK](https://github.com/WAGO/pfc-firmware-sdk-G2)
- [WAGO PFC HowTos](https://github.com/WAGO/pfc-howtos)
