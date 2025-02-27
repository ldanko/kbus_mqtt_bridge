//! KBUS communication module for PFC200 devices
//!
//! This module provides functionality for interfacing with the WAGO PFC200 KBUS system.
//! It handles bidirectional communication with digital I/O modules connected to the controller,
//! providing a thread-safe way to read from and write to digital channels.

use std::{sync::mpsc::Receiver, time::Duration};

use anyhow::Context;
use bitvec::prelude::*;
use kbus::KBus;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::mpsc::UnboundedSender,
    time::{MissedTickBehavior, interval},
};
use tokio_util::sync::CancellationToken;
use tracing::{info, info_span, instrument, warn};

const KBUS_CYCLE: Duration = Duration::from_millis(10);

/// Represents a digital I/O event on the KBUS system.
///
/// This structure is used to communicate events between the KBUS hardware
/// and the application, representing both input and output signals.
#[derive(Debug, Serialize, Deserialize)]
pub struct KBusEvent {
    /// The channel number (0-based) on which the event occurred.
    pub channel: u16,
    /// The boolean state of the channel (true = ON, false = OFF).
    pub value: bool,
}

/// Main task function that handles bidirectional communication with the KBUS hardware.
///
/// This function runs in a separate task and manages both reading from and writing to
/// the KBUS hardware. It uses double-buffering for detecting changes on input channels
/// and forwards these changes to the application through the provided sender.
/// It also processes output events received from the application and writes them to
/// the KBUS hardware.
///
/// # Arguments
///
/// * `input_tx` - Channel for sending input events detected on the KBUS to the application
/// * `kbus_output_rx` - Channel for receiving output events from the application to write to KBUS
/// * `cancellation_token` - Token to signal when this task should terminate
///
/// # Errors
///
/// Returns errors on KBUS initialization, communication, or channel failures.
#[instrument(name = "kbus", skip_all, err)]
pub async fn kbus_task(
    input_tx: UnboundedSender<KBusEvent>,
    kbus_output_rx: Receiver<KBusEvent>,
    cancellation_token: CancellationToken,
) -> Result<(), anyhow::Error> {
    info!("starting K-Bus task");

    // Maximum number of digital input channels to monitor
    let input_size = 90;
    // Maximum number of digital output channels
    let output_size = 90;

    // Double buffer setup for change detection
    // Using two bit vectors to detect changes between KBUS cycles
    let mut buffers = [
        bitvec![u8, LocalBits; 0; input_size],
        bitvec![u8, LocalBits; 0; input_size],
    ];

    // Index of the current buffer (toggles between 0 and 1)
    let mut current_buffer = 0;

    // Initialize KBUS communication
    let mut kbus = KBus::new().context("failed to create K-Bus instance")?;

    // Set application state to "Running" to drive kbus by yourself.
    kbus.start().context("failed ot start K-Bus instanece")?;

    info!("K-Bus task started");

    let mut interval = interval(KBUS_CYCLE);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    // Main processing loop - runs until cancellation is requested
    loop {
        tokio::select! {
            // Wait for next cycle (100 Hz frequency)
            _ = interval.tick() => {},
            _ = cancellation_token.cancelled() => {
                return Ok(())
            },
        }

        // Trigger a hardware bus cycle - reads inputs and writes outputs
        kbus.trigger_bus_cycle()
            .context("failed to trigger K-Bus cycle")?;

        // Process input signals
        {
            let _in_span = info_span!("in").entered();

            // Get the current and previous buffer indices using XOR toggle pattern
            let current = current_buffer;
            let old = current ^ 1; // XOR with 1 toggles between 0 and 1
            current_buffer = old; // Swap for next iteration

            // Read the current state of all input channels into the current buffer
            let mut reader = kbus.reader().context("failed to create K-Bus reader")?;
            reader
                .read_bytes(0, buffers[current].as_raw_mut_slice())
                .context("failed to read from K-Bus")?;

            // Compare current and previous buffer to detect changes
            // Create a temporary bitvec to hold the differences
            let mut diff_bits = buffers[current].clone();
            // XOR with old buffer to find differences (1 means bit changed)
            diff_bits ^= &buffers[old];

            // Iterate through set bits in the diff_bits (only process changed bits)
            for i in diff_bits.iter_ones() {
                // Create and send event for changed channel
                let event = KBusEvent {
                    channel: i as u16,
                    value: buffers[current][i],
                };
                info!(?event);
                input_tx
                    .send(event)
                    .context("K-Bus input processing channel closed")?;
            }
        }

        // Process output signals
        {
            let _out_span = info_span!("out").entered();

            // Get all pending output events, if any
            let mut output_iter = kbus_output_rx.try_iter().peekable();
            if output_iter.peek().is_some() {
                // Create writer for output operations
                let mut writer = kbus.writer().context("failed to create K-Bus writer")?;

                // Process each output event
                for event in output_iter {
                    // Validate channel is within range
                    if event.channel < output_size {
                        info!(?event);
                        writer
                            .write_bool(event.channel as u32, event.value)
                            .context("failed to write to K-Bus")?;
                    } else {
                        warn!("invalid channel {} (max {output_size})", event.channel);
                    }
                }
            }
        }
    }
}
