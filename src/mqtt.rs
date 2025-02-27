use std::{
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::Sender,
    },
    time::{Duration, Instant},
};

use anyhow::Context;
use chrono::Utc;
use mac_address::MacAddress;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, Publish, QoS};
use serde_json::json;
use sysinfo::{MemoryRefreshKind, System};
use tokio::{sync::mpsc::UnboundedReceiver, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, trace, warn};

use crate::kbus::KBusEvent;

static SYSTEM: LazyLock<Mutex<System>> = LazyLock::new(|| {
    let refresh_kind = RefreshKind::new()
        .with_cpu(CpuRefreshKind::new().with_cpu_usage())
        .with_memory(MemoryRefreshKind::new().with_ram());

    let sys = System::new_with_specifics(refresh_kind);

    Mutex::new(sys)
});
static APP_START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);
static MQTT_MESSAGES_RECEIVED: AtomicU64 = AtomicU64::new(0);
static MQTT_MESSAGES_SENT: AtomicU64 = AtomicU64::new(0);
static MQTT_MESSAGES_REJECTED: AtomicU64 = AtomicU64::new(0);
static MQTT_MESSAGES_PROCESSED: AtomicU64 = AtomicU64::new(0);

const fn decode_value(payload: &[u8]) -> Option<bool> {
    match payload {
        b"true" | b"on" | b"\x01" => Some(true),
        b"false" | b"off" | b"\x00" => Some(false),
        _ => None,
    }
}

enum DecodedTopic {
    KBusOutput { channel: u16 },
}

struct MqttEventLoop {
    event_loop: EventLoop,
    topic_prefix: String,
    kbus_output: Sender<KBusEvent>,
}

impl MqttEventLoop {
    fn new(
        event_loop: EventLoop,
        topic_prefix: String,
        kbus_output: Sender<KBusEvent>,
    ) -> MqttEventLoop {
        MqttEventLoop {
            event_loop,
            topic_prefix,
            kbus_output,
        }
    }

    fn decode_topic(&self, topic: &str) -> Option<DecodedTopic> {
        let topic = topic.strip_prefix(&self.topic_prefix)?;
        if let Some(maybe_channel) = topic.strip_prefix("/output/") {
            let channel = maybe_channel.parse().ok()?;
            Some(DecodedTopic::KBusOutput { channel })
        } else {
            None
        }
    }

    fn on_mqtt_message(&mut self, topic: &str, payload: &[u8]) -> Result<(), anyhow::Error> {
        info!(topic, ?payload);

        match self.decode_topic(topic) {
            Some(DecodedTopic::KBusOutput { channel }) => {
                let Some(value) = decode_value(payload) else {
                    let topic_prefix = &self.topic_prefix;
                    warn!("'{topic_prefix}{topic}': invalid payload: {payload:?}");
                    return Ok(());
                };
                let event = KBusEvent { channel, value };
                self.kbus_output
                    .send(event)
                    .context("K-Bus output queue closed")?;
            }
            None => {
                // This should never happen, but even if it does,
                // we can safely ignore it
                warn!("Unknown topic '{topic}");
            }
        }

        Ok(())
    }

    async fn poll(&mut self) -> Result<Event, anyhow::Error> {
        self.event_loop
            .poll()
            .await
            .context("failed to poll MQTT event loop")
    }
}

#[instrument(name = "sub", skip_all, err)]
async fn mqtt_event_loop(event_loop: &mut MqttEventLoop) -> Result<(), anyhow::Error> {
    loop {
        let notification = event_loop.poll().await?;
        trace!(?notification);
        match notification {
            Event::Incoming(Packet::Publish(Publish { topic, payload, .. })) => {
                MQTT_MESSAGES_RECEIVED.fetch_add(1, Ordering::Relaxed);

                if let Err(err) = event_loop.on_mqtt_message(&topic, &payload) {
                    warn!(message_rejected = format!("{err:#}"));
                    MQTT_MESSAGES_REJECTED.fetch_add(1, Ordering::Relaxed);
                } else {
                    MQTT_MESSAGES_PROCESSED.fetch_add(1, Ordering::Relaxed);
                }
            }
            Event::Incoming(_) | Event::Outgoing(_) => {}
        }
    }
}

struct MqttPublisher {
    client: AsyncClient,
    topic_prefix: String,
}

impl MqttPublisher {
    fn new(client: AsyncClient, topic_prefix: String) -> MqttPublisher {
        MqttPublisher {
            client,
            topic_prefix,
        }
    }

    async fn publish(
        &self,
        topic: &str,
        qos: QoS,
        retain: bool,
        payload: String,
    ) -> Result<(), anyhow::Error> {
        let topic_prefix = &self.topic_prefix;
        let topic = if topic_prefix.is_empty() {
            topic.to_owned()
        } else {
            format!("{topic_prefix}/{topic}")
        };

        info!(topic, payload);
        self.client.publish(topic, qos, retain, payload).await?;

        MQTT_MESSAGES_SENT.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    async fn publish_heartbeat(&self) -> Result<(), anyhow::Error> {
        let app_uptime = APP_START_TIME.elapsed().as_secs();

        let mqtt_msgs_in = MQTT_MESSAGES_RECEIVED.load(Ordering::Relaxed);
        let mqtt_msgs_out = MQTT_MESSAGES_SENT.load(Ordering::Relaxed);
        let mqtt_rejected = MQTT_MESSAGES_REJECTED.load(Ordering::Relaxed);
        let mqtt_sent = MQTT_MESSAGES_SENT.load(Ordering::Relaxed);

        let mut system = SYSTEM.lock().unwrap();

        system.refresh_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::new().with_ram()),
        );

        let total_memory = system.total_memory() as f32;
        let used_memory = system.used_memory() as f32;
        let memory_percentage = if total_memory > 0.0 {
            (used_memory / total_memory) * 100.0
        } else {
            0.0
        };

        let payload = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "app_uptime": app_uptime,
            "system_uptime": System::uptime(),
            "cpu_usage": system.global_cpu_usage(),
            "memory_usage": memory_percentage,
            "mqtt_stats": {
                "received": mqtt_received,
                "processed": mqtt_processed,
                "rejected": mqtt_rejected,
                "sent": mqtt_sent,
                "total": mqtt_received + mqtt_sent
            },
        });

        self.publish("heartbeat", QoS::AtLeastOnce, false, payload.to_string())
            .await?;

        Ok(())
    }
}

#[instrument(name = "pub", skip_all, err)]
async fn mqtt_publish_loop(
    mqtt_publisher: &mut MqttPublisher,
    input_events: &mut UnboundedReceiver<KBusEvent>,
) -> Result<(), anyhow::Error> {
    info!("Starting MQTT publish task");

    mqtt_publisher
        .publish("status", QoS::ExactlyOnce, true, "online".to_owned())
        .await?;

    // timeout, after timeout heartbeat (albo status)
    while let Some(event) = input_events.recv().await {
        let topic = format!("input/{}", event.channel);
        let payload = event.value.to_string();
        mqtt_publisher
            .publish(&topic, QoS::AtLeastOnce, false, payload)
            .await?;
    }

    mqtt_publisher
        .publish("status", QoS::ExactlyOnce, true, "offline".to_owned())
        .await?;

    Ok(())
}

#[instrument(name = "mqtt", skip_all, err)]
pub async fn mqtt_client_task(
    mac: MacAddress,
    mqtt_options: MqttOptions,
    input_events: &mut UnboundedReceiver<KBusEvent>,
    kbus_output: Sender<KBusEvent>,
    cancellation_token: CancellationToken,
) -> Result<(), anyhow::Error> {
    loop {
        let (client, event_loop) = AsyncClient::new(mqtt_options.clone(), 10);
        let topic_prefix = format!("pfc200/{mac}");
        client
            .subscribe(format!("{topic_prefix}/output/+"), QoS::ExactlyOnce)
            .await?;

        let mut mqtt_subscriber =
            MqttEventLoop::new(event_loop, topic_prefix.clone(), kbus_output.clone());
        let mut mqtt_publisher = MqttPublisher::new(client, topic_prefix);

        tokio::select! {
            res = mqtt_event_loop(&mut mqtt_subscriber) => {
                res.context("MQTT event loop task failed")?
            },
            res = mqtt_publish_loop(&mut mqtt_publisher, input_events) => {
                res.context("MQTT publish task failed")?
            },
            _ = cancellation_token.cancelled() => break,
        }
    }

    Ok(())
}
