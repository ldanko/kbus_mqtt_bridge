use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

use super::*;

#[tokio::test]
async fn test_kbus_event_processing() {
    tracing_subscriber::fmt::init();

    // Setup channels for testing
    let (input_tx, mut input_rx) = unbounded_channel();
    let (output_tx, output_rx) = unbounded_channel();
    let cancellation_token = CancellationToken::new();

    // Reset mock state before test
    kbus_mock::reset_state();

    // Set an initial input bit in the mock
    kbus_mock::set_input_bit(5, true).unwrap();

    // Start the KBUS task in the background
    let task_handle = tokio::spawn(kbus_task(input_tx, output_rx, cancellation_token.clone()));

    // Wait a bit to let the task initialize and read inputs
    tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;

    // We should receive an event for bit 5 which was set to true
    if let Some(event) = input_rx.recv().await {
        assert_eq!(event.channel, 5);
        assert_eq!(event.value, true);
    } else {
        panic!("Expected to receive an event");
    }

    // Now send an output event
    let output_event = KBusEvent {
        channel: 10,
        value: true,
    };
    output_tx.send(output_event).unwrap();

    // Wait for the event to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;

    // Check if the output was set correctly in the mock
    assert_eq!(kbus_mock::get_output_bit(10).unwrap(), true);

    // Cleanup
    cancellation_token.cancel();
    let _ = task_handle.await;
}
