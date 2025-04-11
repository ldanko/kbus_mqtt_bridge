use std::{
    env,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Configuration for MQTT connection settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MqttConfig {
    /// MQTT broker hostname or IP address
    pub broker_host: String,

    /// MQTT broker port
    #[serde(default = "default_mqtt_port")]
    pub broker_port: u16,

    /// MQTT username for authentication (optional)
    #[serde(default)]
    pub username: Option<String>,

    /// MQTT password for authentication (optional)
    #[serde(default)]
    pub password: Option<String>,

    /// MQTT connection keepalive duration
    #[serde(default = "default_keepalive", with = "humantime_serde")]
    pub keepalive: Duration,

    /// Heartbeat interval duration (how often to send status updates, set to 0 to disable)
    #[serde(default = "default_heartbeat_interval", with = "humantime_serde")]
    pub heartbeat_interval: Duration,
}

/// Main application configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Name of the device to use in MQTT topics
    #[serde(default = "default_device_name")]
    pub device_name: String,

    /// MQTT connection configuration
    pub mqtt: MqttConfig,
}

// Default values

const fn default_mqtt_port() -> u16 {
    1883
}

const fn default_keepalive() -> Duration {
    Duration::from_secs(300) // 5 minutes
}

const fn default_heartbeat_interval() -> Duration {
    Duration::from_secs(60) // 1 minute
}

fn default_device_name() -> String {
    "kbus_mqtt_bridge".to_owned()
}

impl Default for MqttConfig {
    fn default() -> MqttConfig {
        MqttConfig {
            broker_host: "localhost".to_string(),
            broker_port: default_mqtt_port(),
            username: None,
            password: None,
            keepalive: default_keepalive(),
            heartbeat_interval: default_heartbeat_interval(),
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            device_name: default_device_name(),
            mqtt: MqttConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML configuration file
    pub fn from_toml<P: AsRef<Path>>(path: P) -> Result<Config, anyhow::Error> {
        let mut file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open config file: {}", path.as_ref().display()))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse TOML config: {}", path.as_ref().display()))?;

        Ok(config)
    }

    /// Load configuration with the following precedence:
    /// 1. Environment variables
    /// 2. Config file (specified via command line argument)
    /// 3. Config file (specified via KBUS_BRIDGE_CONFIG_FILE environment variable)
    /// 4. Default values
    ///
    /// # Environment Variables
    /// - `KBUS_BRIDGE_DEVICE_NAME`: Device name (default: "kbus_mqtt_bridge")
    /// - `KBUS_BRIDGE_MQTT_HOST`: MQTT broker host
    /// - `KBUS_BRIDGE_MQTT_PORT`: MQTT broker port (default: 1883)
    /// - `KBUS_BRIDGE_MQTT_KEEPALIVE`: MQTT keepalive in seconds (default: 300)
    /// - `KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL`: MQTT heartbeat interval in seconds (default: 60)
    /// - `KBUS_BRIDGE_CONFIG_FILE`: Path to config file (used if command line path not provided)
    ///
    /// # Arguments
    ///
    /// * `config_path` - Optional path to a configuration file from command line
    pub fn load(config_path: Option<PathBuf>) -> Result<Config, anyhow::Error> {
        // Try to get config file path from environment if not provided via command line
        let config_path = config_path.or_else(|| {
            env::var("KBUS_BRIDGE_CONFIG_FILE")
                .ok()
                .as_ref()
                .map(PathBuf::from)
        });

        // Override with config file if provided
        let mut config = if let Some(path) = config_path {
            if path.exists() {
                Config::from_toml(&path)?
            } else {
                return Err(anyhow::anyhow!("Config file not found: {}", path.display()));
            }
        } else {
            Config::default()
        };

        // Override with environment variables if they exist

        if let Ok(device_name) = env::var("KBUS_BRIDGE_DEVICE_NAME") {
            config.device_name = device_name;
        }

        if let Ok(broker_host) = env::var("KBUS_BRIDGE_MQTT_HOST") {
            config.mqtt.broker_host = broker_host;
        }

        if let Ok(username) = env::var("KBUS_BRIDGE_MQTT_USERNAME") {
            config.mqtt.username = Some(username);
        }

        if let Ok(password) = env::var("KBUS_BRIDGE_MQTT_PASSWORD") {
            config.mqtt.password = Some(password);
        }

        if let Ok(port_str) = env::var("KBUS_BRIDGE_MQTT_PORT") {
            if let Ok(port) = port_str.parse::<u16>() {
                config.mqtt.broker_port = port;
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid KBUS_BRIDGE_MQTT_PORT value: {}",
                    port_str
                ));
            }
        }

        if let Ok(keepalive_str) = env::var("KBUS_BRIDGE_MQTT_KEEPALIVE") {
            if let Ok(keepalive) = keepalive_str.parse::<u64>() {
                config.mqtt.keepalive = Duration::from_secs(keepalive);
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid KBUS_BRIDGE_MQTT_KEEPALIVE value: {}",
                    keepalive_str
                ));
            }
        }

        if let Ok(heartbeat_str) = env::var("KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL") {
            if let Ok(heartbeat) = heartbeat_str.parse::<u64>() {
                config.mqtt.heartbeat_interval = Duration::from_secs(heartbeat);
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL value: {}",
                    heartbeat_str
                ));
            }
        }

        // Validate the config before returning
        config.validate()?;
        Ok(config)
    }

    /// Validates the configuration values.
    ///
    /// Returns an error if any configuration value is invalid.
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        // Validate device name (non-empty and no invalid characters)
        if self.device_name.is_empty() {
            return Err(anyhow::anyhow!("Device name cannot be empty"));
        }

        // Check for invalid characters in device name
        // More efficient single-pass check
        for c in self.device_name.chars() {
            let error_msg = match c {
                c if c.is_whitespace() => Some("Device name cannot contain whitespace"),
                '/' => Some("Device name cannot contain '/' character (MQTT topic separator)"),
                '+' => Some("Device name cannot contain '+' character (MQTT topic wildcard)"),
                '#' => Some("Device name cannot contain '#' character (MQTT topic wildcard)"),
                _ => None,
            };

            if let Some(msg) = error_msg {
                return Err(anyhow::anyhow!(msg));
            }
        }

        // Validate MQTT broker host (non-empty)
        if self.mqtt.broker_host.is_empty() {
            return Err(anyhow::anyhow!("MQTT broker host cannot be empty"));
        }

        // Validate port (though any u16 is valid, check specific port ranges)
        if self.mqtt.broker_port == 0 {
            return Err(anyhow::anyhow!("MQTT broker port cannot be 0"));
        }

        // Validate keepalive (shouldn't be too short or too long)
        if self.mqtt.keepalive.as_secs() < 5 {
            return Err(anyhow::anyhow!("MQTT keepalive must be at least 5 seconds"));
        }
        if self.mqtt.keepalive.as_secs() > 86400 {
            return Err(anyhow::anyhow!(
                "MQTT keepalive must be at most 24 hours (86400 seconds)"
            ));
        }

        // Validate heartbeat interval (shouldn't be too long, 0 means disabled)
        // A value of 0 means "don't send heartbeat"
        if !self.mqtt.heartbeat_interval.is_zero() && self.mqtt.heartbeat_interval.as_secs() < 1 {
            return Err(anyhow::anyhow!(
                "Heartbeat interval must be at least 1 second or 0 to disable"
            ));
        }
        if self.mqtt.heartbeat_interval.as_secs() > 3600 {
            return Err(anyhow::anyhow!(
                "Heartbeat interval must be at most 1 hour (3600 seconds)"
            ));
        }

        Ok(())
    }
}
