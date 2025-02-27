use std::{
    collections::HashMap,
    error::Error,
    ffi::CStr,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use anyhow::Context;
use bitvec::prelude::*;
use kbus::KBus;
use mac_address::{MacAddress, get_mac_address};
use rumqttc::{AsyncClient, Event, EventLoop, LastWill, MqttOptions, Packet, Publish, QoS};
use serde_derive::{Deserialize, Serialize};
use tokio::{
    signal,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span, instrument, trace, warn};
use tracing_subscriber;

// priorities
const KBUS_MAINPRIO: i32 = 40; // main loop;

fn configure_scheduler(policy: i32, priority: i32) -> Result<(), anyhow::Error> {
    let s_param = libc::sched_param {
        sched_priority: priority,
    };
    unsafe {
        if libc::sched_setscheduler(0, policy, &s_param) == -1 {
            let errno = *libc::__errno_location();
            let error_msg = CStr::from_ptr(libc::strerror(errno)).to_string_lossy();
            anyhow::bail!("sched_setscheduler call failed: {error_msg}");
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(from = "InputConfigDef")]
struct InputConfig {
    topic: String,
    #[serde(default)]
    retain: bool,
    #[serde(default)]
    timestamp: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum InputConfigDef {
    Simple(String),
    Full(InputConfig),
}

impl From<InputConfigDef> for InputConfig {
    fn from(config: InputConfigDef) -> Self {
        match config {
            InputConfigDef::Simple(topic) => InputConfig {
                topic,
                retain: false,
                timestamp: false,
            },
            InputConfigDef::Full(input_config) => input_config,
        }
    }
}

mod qos_number {
    use rumqttc::QoS;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(qos: &QoS, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*qos as u8)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<QoS, D::Error>
    where
        D: Deserializer<'de>,
    {
        let num = u8::deserialize(deserializer)?;
        match num {
            0 => Ok(QoS::AtMostOnce),
            1 => Ok(QoS::AtLeastOnce),
            2 => Ok(QoS::ExactlyOnce),
            _ => Err(serde::de::Error::custom("Invalid QoS value")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(from = "OutputConfigDef")]
struct OutputConfig {
    topic: String,
    #[serde(with = "qos_number")]
    //#[serde(serialize_with = "qos_number::serialize", deserialize_with = "qos_number::deserialize")]
    qos: QoS,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OutputConfigDef {
    Simple(String),
    Full(OutputConfig),
}

impl From<OutputConfigDef> for OutputConfig {
    fn from(config: OutputConfigDef) -> Self {
        match config {
            OutputConfigDef::Simple(topic) => OutputConfig {
                topic,
                qos: QoS::AtLeastOnce,
            },
            OutputConfigDef::Full(output_config) => output_config,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MqttConfig {
    broker_host: String,
    broker_port: u16,
    keepalive: Duration,
    reconnect: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    device_name: String,
    mqtt: MqttConfig,
    inputs: HashMap<u16, InputConfig>,
    output: HashMap<u16, OutputConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KBusEvent {
    offset: u16,
    value: bool,
}

#[derive(Debug, Serialize, Deserialize)]
enum Status {
    Online,
    Offline,
}

#[derive(Debug, Serialize, Deserialize)]
enum Message {
    Start,
    Stop,
    Status(Status),
    KBusEvent(KBusEvent),
}

#[instrument(skip_all, err)]
fn kbus_thread(
    input_tx: UnboundedSender<KBusEvent>,
    output_rx: Receiver<KBusEvent>,
    cancellation_token: CancellationToken,
) -> Result<(), anyhow::Error> {
    info!("starting K-Bus thread");

    let input_size = 90;
    let output_size = 90;

    let mut buffers = [
        bitvec![u8, LocalBits; 0; input_size],
        bitvec![u8, LocalBits; 0; input_size],
    ];

    let mut buffer_switcher = [(0, 1), (1, 0)].iter().copied().cycle();

    let mut kbus = KBus::new().context("failed to create K-Bus instance")?;

    // Set application state to "Running" to drive kbus by yourself.
    kbus.start().context("failed ot start K-Bus instanece")?;

    info!("K-Bus thread started");

    while !cancellation_token.is_cancelled() {
        thread::sleep(Duration::from_millis(10));

        kbus.trigger_bus_cycle()
            .context("failed to trigger K-Bus cycle")?;

        {
            let _in_span = info_span!("in").entered();

            let Some((current, old)) = buffer_switcher.next() else {
                unreachable!();
            };

            let mut reader = kbus.reader().context("failed to create K-Bus reader")?;
            reader
                .read_bytes(0, buffers[current].as_raw_mut_slice())
                .context("failed to read from K-Bus")?;

            for (i, (c, o)) in buffers[current].iter().zip(buffers[old].iter()).enumerate() {
                if *c ^ *o {
                    let event = KBusEvent {
                        offset: i as u16,
                        value: *c,
                    };
                    info!(?event);
                    input_tx
                        .send(event)
                        .context("K-Bus input processing channel closed")?;
                }
            }
        }

        {
            let _out_span = info_span!("out").entered();
            let mut output_iter = output_rx.try_iter().peekable();
            if output_iter.peek().is_some() {
                let mut writer = kbus.writer().context("failed to create K-Bus writer")?;
                for event in output_iter {
                    if event.offset < output_size {
                        info!(?event);
                        writer
                            .write_bool(event.offset as u32, event.value)
                            .context("failed to write to K-Bus")?;
                    } else {
                        warn!("invalid offset {} (max {output_size})", event.offset);
                    }
                }
            }
        }
    }
    Ok(())
}

#[instrument(name="sub",skip_all, err)]
async fn mqtt_event_loop_task(
    mut event_loop: EventLoop,
    topic_prefix: &str,
    output_events: Sender<KBusEvent>,
) -> Result<(), anyhow::Error> {
    loop {
        let notification = event_loop
            .poll()
            .await
            .context("failed to poll MQTT event loop")?;
        trace!(?notification);
        match notification {
            Event::Incoming(Packet::Publish(Publish { topic, payload, .. })) => {
                info!(topic, ?payload);
                let Some(topic) = topic.strip_prefix(&topic_prefix) else {
                    debug!("topic prefix does not match '{topic_prefix}");
                    continue;
                };

                if let Some(offset) = topic.strip_prefix("/output/") {
                    let Ok(offset) = u16::from_str_radix(offset, 10) else {
                        debug!("unexpected topic '{topic_prefix}{topic}");
                        continue;
                    };
                    let value = match payload.as_ref() {
                        b"true" | b"on" | b"\x01" => true,
                        b"false" | b"off" | b"\x00" => false,
                        _ => {
                            warn!("'{topic_prefix}{topic}': invalid payload: {payload:?}");
                            continue;
                        }
                    };
                    let event = KBusEvent { offset, value };
                    output_events.send(event)?;
                } else if topic.starts_with("/config/mapping") {
                    debug!("IO mapping not implemented yet");
                } else {
                    debug!("unexpected topic '{topic_prefix}{topic}'");
                }
            }
            _ => {}
        }
    }
}

#[instrument(name = "pub", skip_all, err)]
async fn mqtt_publish_task(
    client: AsyncClient,
    topic_prefix: &str,
    input_events: &mut UnboundedReceiver<KBusEvent>,
) -> Result<(), anyhow::Error> {
    info!("Starting MQTT publish task");
    client
        .publish(
            format!("{topic_prefix}/status"),
            QoS::ExactlyOnce,
            true,
            "online",
        )
        .await
        .with_context(|| format!("failed to publish '{topic_prefix}/status'"))?;

    // timeout, after timeout heartbeat (albo status)
    while let Some(event) = input_events.recv().await {
        let topic = format!("{topic_prefix}/input/{}", event.offset);
        let payload = event.value.to_string();
        info!(topic, payload);
        client
            .publish(topic, QoS::AtLeastOnce, true, payload)
            .await?;
    }

    client
        .publish(
            format!("{topic_prefix}/status"),
            QoS::ExactlyOnce,
            true,
            "offline",
        )
        .await
        .with_context(|| format!("failed to publish '{topic_prefix}/status'"))?;

    Ok(())
}

/*
 *
 * Status

use serde_json::json;
use chrono::Utc;

fn publish_status(mqtt_client: &MqttClient, online: bool, uptime: u64, cpu: f32, mem: f32, mqtt_msgs: u64) {
    let payload = json!({
        "status": if online { "online" } else { "offline" },
        "timestamp": Utc::now().to_rfc3339(),
        "uptime": uptime,
        "cpu_usage": cpu,
        "memory_usage": mem,
        "mqtt_messages": mqtt_msgs
    });

    mqtt_client.publish(
        "pfc200/00:30:DE:48:18:71/status",
        QoS::AtLeastOnce,
        true, // Retain = true, żeby broker pamiętał status
        payload.to_string()
    );
}
*/

#[instrument(name="mqtt", skip_all, err)]
async fn mqtt_client_task(
    mac: MacAddress,
    mqtt_options: MqttOptions,
    input_events: &mut UnboundedReceiver<KBusEvent>,
    output_events: Sender<KBusEvent>,
    cancellation_token: CancellationToken,
) -> Result<(), anyhow::Error> {
    let (client, event_loop) = AsyncClient::new(mqtt_options, 10);
    let topic_prefix = format!("pfc200/{mac}");
    client
        .subscribe(format!("{topic_prefix}/config/mapping"), QoS::ExactlyOnce)
        .await?;
    client
        .subscribe(format!("{topic_prefix}/output/+"), QoS::ExactlyOnce)
        .await?;

    tokio::select! {
        res = mqtt_event_loop_task(event_loop, &topic_prefix, output_events) => res.context("MQTT event loop task failed")?,
        res = mqtt_publish_task(client, &topic_prefix, input_events) => res.context("MQTT publish task failed")?,
        _ = cancellation_token.cancelled() => {},
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var("RUST_LOG").is_err() {
        let rust_log = "info,kbus_mqtt_bridge=info";
        unsafe { std::env::set_var("RUST_LOG", &rust_log) };
    }

    tracing_subscriber::fmt::init();

    // switch to RT Priority
    configure_scheduler(libc::SCHED_FIFO, KBUS_MAINPRIO)
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
    let (output_tx, output_rx) = mpsc::channel();

    let kbus_thread_handle = tokio::task::spawn_blocking({
        let cancellation_token = cancellation_token.clone();
        move || {
            if let Err(err) = kbus_thread(input_tx, output_rx, cancellation_token.clone()) {
                error!("K-Bus thread failed: {err:#}");
            }
            cancellation_token.cancel();
        }
    });

    let mqtt_task_handle = tokio::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            loop {
                if let Err(err) = mqtt_client_task(
                    mac.clone(),
                    mqtt_options.clone(),
                    &mut input_rx,
                    output_tx.clone(),
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

                thread::sleep(Duration::from_secs(3));
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

    kbus_thread_handle
        .await
        .context("failed to join K-Bus thread")?;

    mqtt_task_handle.await.context("failed to join MQTT task")?;

    Ok(())
}
