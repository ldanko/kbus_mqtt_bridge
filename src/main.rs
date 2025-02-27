use std::{
    collections::HashMap,
    env,
    error::Error,
    sync::mpsc::{self},
    time::Duration,
};

use anyhow::Context;
use kbus_mqtt_bridge::{
    kbus::{KBusEvent, kbus_task},
    mqtt::mqtt_client_task,
    utils::{KBUS_MAINPRIO, SchedPolicy, configure_scheduler},
};
use mac_address::get_mac_address;
use rumqttc::{LastWill, MqttOptions, QoS};
use serde::Deserialize;
use tokio::{signal, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
struct MqttConfig {
    broker_host: String,
    broker_port: u16,
    keepalive: Duration,
    reconnect: Duration,
}

#[derive(Debug, Deserialize)]
struct Config {
    device_name: String,
    mqtt: MqttConfig,
}

#[derive(Debug, Deserialize)]
enum Status {
    Online,
    Offline,
}

#[derive(Debug, Deserialize)]
enum Message {
    Start,
    Stop,
    Status(Status),
    KBusEvent(KBusEvent),
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

    // switch to RT Priority
    configure_scheduler(SchedPolicy::Fifo, KBUS_MAINPRIO)
        .context("failed to set scheduler priority")?;

    let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
        .context("failed to setup SIGTERM handler")?;

    let cancellation_token = CancellationToken::new();

    let mac = get_mac_address()
        .context("Failed to retrieve MAC address")?
        .ok_or_else(|| anyhow::anyhow!("No MAC address found"))?;

    let mut mqtt_options = MqttOptions::new("pfc200", "mqtt-broker", 1883);
    mqtt_options.set_keep_alive(Duration::from_secs(300));
    mqtt_options.set_last_will(LastWill {
        topic: format!("pfc200/{mac}/status"),
        message: "offline".into(),
        qos: QoS::ExactlyOnce,
        retain: true,
    });

    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (kbus_output_tx, kbus_output_rx) = mpsc::channel();

    let kbus_task_handle = tokio::task::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            if let Err(err) = kbus_task(input_tx, kbus_output_rx, cancellation_token.clone()).await
            {
                error!("K-Bus task failed: {err:#}");
            }
            cancellation_token.cancel();
        }
    });

    let mqtt_task_handle = tokio::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            loop {
                if let Err(err) = mqtt_client_task(
                    mac,
                    mqtt_options.clone(),
                    &mut input_rx,
                    kbus_output_tx.clone(),
                    cancellation_token.clone(),
                )
                .await
                {
                    error!("MQTT client disconnected: {err:?}");
                }

                if cancellation_token.is_cancelled() {
                    break;
                }

                info!("reconnecting in 3s");

                sleep(Duration::from_secs(3)).await;
            }
        }
    });

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
        .context("failed to join K-Bus task")?;

    mqtt_task_handle.await.context("failed to join MQTT task")?;

    Ok(())
}
