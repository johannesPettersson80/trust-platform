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

use trust_runtime::debug::{location_to_line_col, DebugLog, DebugStop};
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoSnapshot;
use trust_runtime::value::Duration;
use trust_runtime::RuntimeMetadata;

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

fn cycle_time_hint(metadata: &RuntimeMetadata) -> Duration {
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
