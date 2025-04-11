use std::{env, error::Error, path::PathBuf, time::Duration};

use anyhow::Context;
use kbus_mqtt_bridge::{
    config::Config,
    kbus::kbus_task,
    mqtt::mqtt_client_task,
    utils::{KBUS_MAINPRIO, SchedPolicy, configure_scheduler},
};
use pnet::datalink;
use rumqttc::{LastWill, MqttOptions, QoS};
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

fn print_help() {
    println!("KBUS MQTT Bridge");
    println!("Usage: kbus_mqtt_bridge [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -c, --config <FILE>  Path to TOML configuration file");
    println!("  -h, --help           Print this help message");
    println!("  -v, --version        Print version information");
    println!();
    println!("Configuration can also be provided via environment variables:");
    println!("  KBUS_BRIDGE_CONFIG_FILE     Path to configuration file (alternative to --config)");
    println!("  KBUS_BRIDGE_DEVICE_NAME     Device name used in MQTT topics");
    println!("  KBUS_BRIDGE_MQTT_HOST       MQTT broker hostname or IP address");
    println!("  KBUS_BRIDGE_MQTT_PORT       MQTT broker port");
    println!("  KBUS_BRIDGE_MQTT_USERNAME   MQTT username for authentication");
    println!("  KBUS_BRIDGE_MQTT_PASSWORD   MQTT password for authentication");
    println!("  KBUS_BRIDGE_MQTT_KEEPALIVE  MQTT keepalive duration in seconds");
}

async fn app(config: Config) -> Result<(), anyhow::Error> {
    let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
        .context("failed to setup SIGTERM handler")?;

    let cancellation_token = CancellationToken::new();

    let mac = datalink::interfaces()
        .first()
        .context("No network interface found")?
        .mac
        .context("No MAC address found")?;

    let device_name = config.device_name.clone();
    let topic_prefix = format!("{device_name}/{mac}");

    let mut mqtt_options = MqttOptions::new(
        config.device_name,
        config.mqtt.broker_host,
        config.mqtt.broker_port,
    );
    mqtt_options.set_keep_alive(config.mqtt.keepalive);
    mqtt_options.set_last_will(LastWill {
        topic: format!("{topic_prefix}/status"),
        message: "offline".into(),
        qos: QoS::ExactlyOnce,
        retain: true,
    });

    if let (Some(username), Some(password)) = (&config.mqtt.username, &config.mqtt.password) {
        mqtt_options.set_credentials(username, password);
    }

    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (kbus_output_tx, kbus_output_rx) = tokio::sync::mpsc::unbounded_channel();

    let kbus_task_handle = tokio::task::spawn(kbus_task(
        input_tx,
        kbus_output_rx,
        cancellation_token.clone(),
    ));

    let mqtt_task_handle = tokio::spawn(mqtt_client_task(
        topic_prefix.clone(),
        mqtt_options.clone(),
        input_rx,
        kbus_output_tx.clone(),
        Duration::from_secs(60),
        cancellation_token.clone(),
    ));

    tokio::select! {
        res = signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
            res.context("Unable to listen for shutdown signal")?;
            cancellation_token.cancel();
        },
        _ = terminate.recv() => {
            info!("Received SIGTERM, shutting down...");
            cancellation_token.cancel();
        },
        _ = cancellation_token.cancelled() => {}
    }

    kbus_task_handle
        .await
        .context("failed to join K-Bus task")?
        .context("K-Bus task failed")?;

    mqtt_task_handle
        .await
        .context("failed to join MQTT task")?
        .context("MQTT task failed")?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if env::var("RUST_LOG").is_err() {
        let rust_log = "info,kbus_mqtt_bridge=info";
        // SAFETY: set_var is called during app initialization when no other
        //         threads are running yet. This assumes tokio runtime doesn't
        //         access environment variables during initialization.
        unsafe { env::set_var("RUST_LOG", rust_log) };
    }

    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|arg| arg == "-v" || arg == "--version") {
        println!("KBUS MQTT Bridge v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let config_path = args
        .iter()
        .position(|arg| arg == "-c" || arg == "--config")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from);

    let config = Config::load(config_path)?;
    info!(?config);

    // switch to RT Priority
    configure_scheduler(SchedPolicy::Fifo, KBUS_MAINPRIO)
        .context("failed to set scheduler priority")?;

    if let Err(err) = app(config).await {
        error!(error = format!("{err:#}"));
    }

    Ok(())
}
