use std::fs;

use tempfile::tempdir;

use super::*;

// Helper functions for safely setting/removing environment variables in tests
fn set_env_var(key: &str, value: &str) {
    unsafe { env::set_var(key, value) }
}

fn remove_env_var(key: &str) {
    unsafe { env::remove_var(key) }
}

#[test]
fn test_default_config() {
    let config = Config::default();
    assert_eq!(config.device_name, "kbus_mqtt_bridge");
    assert_eq!(config.mqtt.broker_host, "localhost");
    assert_eq!(config.mqtt.broker_port, 1883);
    assert_eq!(config.mqtt.keepalive, Duration::from_secs(300));
    assert_eq!(config.mqtt.heartbeat_interval, Duration::from_secs(60));
}

#[test]
fn test_from_toml() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    let toml_content = r#"
        device_name = "test_device"

        [mqtt]
        broker_host = "test.mosquitto.org"
        broker_port = 8883
        keepalive = "60s"
        heartbeat_interval = "30s"
        "#;

    fs::write(&config_path, toml_content).unwrap();

    let config = Config::from_toml(config_path).unwrap();
    assert_eq!(config.device_name, "test_device");
    assert_eq!(config.mqtt.broker_host, "test.mosquitto.org");
    assert_eq!(config.mqtt.broker_port, 8883);
    assert_eq!(config.mqtt.keepalive, Duration::from_secs(60));
    assert_eq!(config.mqtt.heartbeat_interval, Duration::from_secs(30));
}

#[test]
fn test_env_variables() {
    // Setup
    set_env_var("KBUS_BRIDGE_DEVICE_NAME", "env_device");
    set_env_var("KBUS_BRIDGE_MQTT_HOST", "env.mqtt.com");
    set_env_var("KBUS_BRIDGE_MQTT_PORT", "2345");
    set_env_var("KBUS_BRIDGE_MQTT_KEEPALIVE", "150");
    set_env_var("KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL", "45");

    let config = Config::load(None).unwrap();
    assert_eq!(config.device_name, "env_device");
    assert_eq!(config.mqtt.broker_host, "env.mqtt.com");
    assert_eq!(config.mqtt.broker_port, 2345);
    assert_eq!(config.mqtt.keepalive, Duration::from_secs(150));
    assert_eq!(config.mqtt.heartbeat_interval, Duration::from_secs(45));

    // Cleanup
    remove_env_var("KBUS_BRIDGE_DEVICE_NAME");
    remove_env_var("KBUS_BRIDGE_MQTT_HOST");
    remove_env_var("KBUS_BRIDGE_MQTT_PORT");
    remove_env_var("KBUS_BRIDGE_MQTT_KEEPALIVE");
    remove_env_var("KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL");
}

#[test]
fn test_load_precedence() {
    // Create config file
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    let toml_content = r#"
        device_name = "file_device"

        [mqtt]
        broker_host = "file.mqtt.org"
        broker_port = 8883
        keepalive = "60s"
        heartbeat_interval = "90s"
        "#;

    fs::write(&config_path, toml_content).unwrap();

    // Create another config file for environment variable
    let env_config_path = dir.path().join("env_config.toml");

    let env_toml_content = r#"
        device_name = "env_file_device"

        [mqtt]
        broker_host = "env_file.mqtt.org"
        broker_port = 7777
        keepalive = "30s"
        heartbeat_interval = "120s"
        "#;

    fs::write(&env_config_path, env_toml_content).unwrap();

    // Set environment variables
    set_env_var("KBUS_BRIDGE_CONFIG_FILE", env_config_path.to_str().unwrap());
    set_env_var("KBUS_BRIDGE_MQTT_HOST", "env.mqtt.com");
    set_env_var("KBUS_BRIDGE_MQTT_PORT", "2345");

    // Test 1: CLI arg takes precedence over env file
    let config = Config::load(Some(config_path.clone())).unwrap();

    // Environment variables should override file config
    assert_eq!(config.device_name, "file_device"); // From CLI config file, not env file
    assert_eq!(config.mqtt.broker_host, "env.mqtt.com"); // Overridden by env var
    assert_eq!(config.mqtt.broker_port, 2345); // Overridden by env var
    assert_eq!(config.mqtt.keepalive, Duration::from_secs(60)); // From CLI config file
    assert_eq!(config.mqtt.heartbeat_interval, Duration::from_secs(90)); // From CLI config file

    // Test 2: Env file used when no CLI arg is provided
    remove_env_var("KBUS_BRIDGE_MQTT_HOST");
    remove_env_var("KBUS_BRIDGE_MQTT_PORT");
    set_env_var("KBUS_BRIDGE_MQTT_KEEPALIVE", "45");
    set_env_var("KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL", "75");

    let config2 = Config::load(None).unwrap();
    assert_eq!(config2.device_name, "env_file_device"); // From env file
    assert_eq!(config2.mqtt.broker_host, "env_file.mqtt.org"); // From env file
    assert_eq!(config2.mqtt.broker_port, 7777); // From env file
    assert_eq!(config2.mqtt.keepalive, Duration::from_secs(45)); // Overridden by env var
    assert_eq!(config2.mqtt.heartbeat_interval, Duration::from_secs(75)); // Overridden by env var

    // Cleanup
    remove_env_var("KBUS_BRIDGE_CONFIG_FILE");
    remove_env_var("KBUS_BRIDGE_MQTT_KEEPALIVE");
    remove_env_var("KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL");
}

#[test]
fn test_invalid_env_values() {
    set_env_var("KBUS_BRIDGE_MQTT_PORT", "not_a_number");
    let result = Config::load(None);
    assert!(result.is_err());

    remove_env_var("KBUS_BRIDGE_MQTT_PORT");
    set_env_var("KBUS_BRIDGE_MQTT_KEEPALIVE", "invalid");
    let result = Config::load(None);
    assert!(result.is_err());

    remove_env_var("KBUS_BRIDGE_MQTT_KEEPALIVE");
    set_env_var("KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL", "invalid");
    let result = Config::load(None);
    assert!(result.is_err());

    remove_env_var("KBUS_BRIDGE_MQTT_HEARTBEAT_INTERVAL");
}

#[test]
fn test_valid_config_validation() {
    let config = Config {
        device_name: "test_device".to_string(),
        mqtt: MqttConfig {
            broker_host: "mqtt.example.com".to_string(),
            broker_port: 1883,
            keepalive: Duration::from_secs(300),
            heartbeat_interval: Duration::from_secs(60),
        },
    };

    let result = config.validate();
    assert!(result.is_ok());
}

#[test]
fn test_invalid_device_name() {
    // Empty device name
    let config = Config {
        device_name: "".to_string(),
        mqtt: MqttConfig::default(),
    };
    let result = config.validate();
    assert!(result.is_err());

    // Device name with spaces
    let config = Config {
        device_name: "test device".to_string(),
        mqtt: MqttConfig::default(),
    };
    let result = config.validate();
    assert!(result.is_err());

    // Device name with MQTT topic separator
    let config = Config {
        device_name: "test/device".to_string(),
        mqtt: MqttConfig::default(),
    };
    let result = config.validate();
    assert!(result.is_err());

    // Device name with MQTT single-level wildcard
    let config = Config {
        device_name: "test+device".to_string(),
        mqtt: MqttConfig::default(),
    };
    let result = config.validate();
    assert!(result.is_err());

    // Device name with MQTT multi-level wildcard
    let config = Config {
        device_name: "test#device".to_string(),
        mqtt: MqttConfig::default(),
    };
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_invalid_mqtt_host() {
    let config = Config {
        device_name: "test_device".to_string(),
        mqtt: MqttConfig {
            broker_host: "".to_string(),
            broker_port: 1883,
            keepalive: Duration::from_secs(300),
            heartbeat_interval: Duration::from_secs(60),
        },
    };
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_invalid_mqtt_port() {
    let config = Config {
        device_name: "test_device".to_string(),
        mqtt: MqttConfig {
            broker_host: "mqtt.example.com".to_string(),
            broker_port: 0,
            keepalive: Duration::from_secs(300),
            heartbeat_interval: Duration::from_secs(60),
        },
    };
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_invalid_keepalive() {
    // Too short keepalive
    let config = Config {
        device_name: "test_device".to_string(),
        mqtt: MqttConfig {
            broker_host: "mqtt.example.com".to_string(),
            broker_port: 1883,
            keepalive: Duration::from_secs(3),
            heartbeat_interval: Duration::from_secs(60),
        },
    };
    let result = config.validate();
    assert!(result.is_err());

    // Too long keepalive
    let config = Config {
        device_name: "test_device".to_string(),
        mqtt: MqttConfig {
            broker_host: "mqtt.example.com".to_string(),
            broker_port: 1883,
            keepalive: Duration::from_secs(100000),
            heartbeat_interval: Duration::from_secs(60),
        },
    };
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_invalid_heartbeat_interval() {
    // Too short heartbeat interval
    let config = Config {
        device_name: "test_device".to_string(),
        mqtt: MqttConfig {
            broker_host: "mqtt.example.com".to_string(),
            broker_port: 1883,
            keepalive: Duration::from_secs(300),
            heartbeat_interval: Duration::from_millis(500),
        },
    };
    let result = config.validate();
    assert!(result.is_err());

    // Too long heartbeat interval
    let config = Config {
        device_name: "test_device".to_string(),
        mqtt: MqttConfig {
            broker_host: "mqtt.example.com".to_string(),
            broker_port: 1883,
            keepalive: Duration::from_secs(300),
            heartbeat_interval: Duration::from_secs(4000),
        },
    };
    let result = config.validate();
    assert!(result.is_err());
}
