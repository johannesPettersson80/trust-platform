//! Adapter core loop + request dispatch.
//! - DebugAdapter::new/session accessors
//! - run/run_with_stdio: protocol loop
//! - dispatch_request/handle_request: route DAP requests
//! - event helpers: output/stopped/terminated

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{self, BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use serde::Serialize;
use serde_json::Value;

use trust_runtime::debug::{location_to_line_col, DebugLog, DebugMode, DebugStop};
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoSnapshot;
use trust_runtime::simulation::SimulationController;
use trust_runtime::value::Duration;
use trust_runtime::{Runtime, RuntimeMetadata};

use crate::protocol::{
    Breakpoint, BreakpointEventBody, Event, MessageType, OutputEventBody, Request, Response,
    SetBreakpointsArguments, SetBreakpointsResponseBody, Source, StoppedEventBody,
};
use crate::runtime::DebugRuntime;

use super::io::io_state_from_snapshot;
use super::protocol_io::{read_message, write_message_locked, write_protocol_log};
use super::remote::RemoteStop;
use super::stop::StopCoordinator;
use super::util::env_flag;
use super::{CoordinateConverter, DebugAdapter, DispatchOutcome, LaunchState, StopGate};

const IO_EVENT_MIN_INTERVAL: StdDuration = StdDuration::from_millis(150);

include!("core_impl_part_01.rs");
include!("core_impl_part_02.rs");
include!("core_impl_part_03.rs");
include!("core_impl_part_04.rs");

#[derive(Debug)]
pub(super) struct DebugRunner {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
    control: trust_runtime::debug::DebugControl,
}

impl DebugRunner {
    pub(super) fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        self.control.clear_breakpoints();
        self.control.continue_run();
        let _ = self.handle.join();
    }
}

/// The cycle of the managed runner: the smallest positive task interval of the loaded program, or 10 ms when no task has one
/// (specification 13). The runner paces by it and advances process time by it for each cycle, and `control.status` reports
/// the same value as `cycle_interval_ms` (annex E1.3), so both read it from here.
pub(super) fn cycle_time_hint(metadata: &RuntimeMetadata) -> Duration {
    metadata
        .tasks()
        .iter()
        .map(|task| task.interval)
        .filter(|interval| interval.as_nanos() > 0)
        .min()
        .unwrap_or_else(|| Duration::from_millis(10))
}

fn wall_interval_for_cycle(cycle_time: Duration) -> StdDuration {
    let nanos = cycle_time.as_nanos();
    if nanos <= 0 {
        return StdDuration::from_millis(10);
    }
    let nanos = u64::try_from(nanos).unwrap_or(u64::MAX);
    StdDuration::from_nanos(nanos)
}

/// Runs one simulator cycle. Process time advances by one exact step only after a cycle that succeeds, so a cycle that
/// the debugger pauses does not advance it (specification 33, §6.1 items 1 and 4).
pub(super) fn execute_simulator_cycle(
    runtime: &mut Runtime,
    simulation: &mut Option<SimulationController>,
    cycle_time: Duration,
) {
    let now = runtime.current_time();
    let cycle_result = if let Some(simulation) = simulation.as_mut() {
        simulation
            .apply_pre_cycle(now, &mut *runtime)
            .and_then(|()| runtime.execute_cycle())
            .and_then(|()| simulation.apply_post_cycle(now, &*runtime))
    } else {
        runtime.execute_cycle()
    };
    match cycle_result {
        Ok(()) => runtime.advance_time(cycle_time),
        Err(err) => {
            if !matches!(err, RuntimeError::InvalidControlFlow) {
                eprintln!("runtime cycle error: {err}");
            }
        }
    }
}

fn sleep_until_or_stopped(stop_flag: &AtomicBool, deadline: Instant) {
    const MAX_SLEEP_CHUNK: StdDuration = StdDuration::from_millis(5);

    while !stop_flag.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.duration_since(now);
        thread::sleep(remaining.min(MAX_SLEEP_CHUNK));
    }
}
