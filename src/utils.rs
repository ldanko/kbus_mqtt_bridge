/// Utility functions and constants for the KBUS MQTT bridge.
///
/// This module provides utilities for system configuration and constants
/// used throughout the application, particularly for scheduler settings.
use std::io;

/// Scheduling policies available for process scheduling.
///
/// These correspond to the Linux scheduling policies defined in `sched.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SchedPolicy {
    /// First-in, first-out real-time scheduling policy.
    ///
    /// Processes scheduled with FIFO policy will run until they complete
    /// or voluntarily yield, unless preempted by a higher-priority process.
    Fifo = libc::SCHED_FIFO,

    /// Round-robin real-time scheduling policy.
    ///
    /// Similar to FIFO but with time-slicing between processes of equal priority.
    RoundRobin = libc::SCHED_RR,

    /// Standard time-sharing scheduling policy.
    ///
    /// Default Linux scheduling policy for normal processes.
    Other = libc::SCHED_OTHER,

    /// Batch scheduling policy for CPU-intensive background work.
    Batch = libc::SCHED_BATCH,

    /// Scheduling policy for very low priority background tasks.
    Idle = libc::SCHED_IDLE,

    /// Deadline scheduling policy for periodic real-time tasks.
    Deadline = libc::SCHED_DEADLINE,
}

/// # Priority Constants
/// Priority values used for configuring the scheduler.
///
/// ## Constants
/// * `KBUS_MAINPRIO` - Priority level (40) for the main KBUS processing loop.
pub const KBUS_MAINPRIO: i32 = 40;

/// Configures the process scheduler with the specified policy and priority.
///
/// This function sets the scheduling policy and priority for the current process
/// using the Linux scheduler interface. It's typically used to set real-time
/// priorities for time-critical operations like KBUS communication.
///
/// # Arguments
///
/// * `policy` - The scheduling policy to set (as a `SchedPolicy` enum value)
/// * `priority` - The priority level to assign (higher values = higher priority)
///
/// # Errors
///
/// May return an `io::Error` if the scheduler cannot be configured. Common error cases include:
/// * Permission denied (EPERM) - The calling process lacks the required privileges
/// * Invalid argument (EINVAL) - The specified policy or priority value is invalid
///
/// # Safety
///
/// This function contains unsafe code to interface with the Linux scheduler API.
/// It requires appropriate permissions to set real-time priorities.
pub fn configure_scheduler(policy: SchedPolicy, priority: i32) -> Result<(), io::Error> {
    let s_param = libc::sched_param {
        sched_priority: priority,
    };
    if unsafe { libc::sched_setscheduler(0, policy as i32, &s_param) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
