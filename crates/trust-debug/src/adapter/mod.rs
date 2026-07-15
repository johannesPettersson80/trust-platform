//! Debug adapter module map.
//! - core: main loop, dispatch, protocol events
//! - handlers: DAP request handlers by area
//! - variables: variable/evaluate/set logic
//! - io: IO state/write handling
//! - protocol_io: message framing + logging
//! - launch: launch argument helpers
//! - util: small shared helpers
//! - tests: adapter unit tests

mod control_bridge;
mod core;
mod handlers;
mod io;
mod launch;
mod paused;
mod protocol_io;
mod remote;
mod stop;
mod stop_remote;
mod util;
mod variables;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufWriter;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use serde_json::Value;

use trust_runtime::eval::expr::Expr;
use trust_runtime::memory::{FrameId, InstanceId};
use trust_runtime::value::{ArrayValue, StructValue, ValueRef};

use crate::protocol::{AttachArguments, IoStateEventBody, LaunchArguments, Request};
use crate::runtime::DebugRuntime;

use self::control_bridge::DebugControlServer;
use self::core::DebugRunner;
use self::paused::PausedStateView;
use self::remote::RemoteSession;
use self::stop_remote::RemoteStopPoller;

#[derive(Debug, Clone)]
enum VariableHandle {
    Locals(FrameId),
    Globals,
    Retain,
    Instances,
    Instance(InstanceId),
    Struct(StructValue),
    Array(ArrayValue),
    Reference(ValueRef),
    IoRoot,
    IoInputs,
    IoOutputs,
    IoMemory,
}

#[derive(Debug, Clone, Copy, Default)]
struct LaunchActions {
    pause_after_launch: bool,
    start_runner_after_launch: bool,
}

#[derive(Debug)]
struct PendingLaunch {
    request: Request<Value>,
    args: LaunchArguments,
    since: Instant,
}

#[derive(Debug)]
struct PendingAttach {
    request: Request<Value>,
    args: AttachArguments,
    since: Instant,
}

#[derive(Debug)]
enum PendingStart {
    Launch(PendingLaunch),
    Attach(PendingAttach),
}

#[derive(Debug)]
enum LaunchState {
    AwaitingConfig { pending: Option<PendingStart> },
    Configured,
    PostLaunch { actions: LaunchActions },
}

impl Default for LaunchState {
    fn default() -> Self {
        Self::AwaitingConfig { pending: None }
    }
}

impl LaunchState {
    fn is_configured(&self) -> bool {
        matches!(self, Self::Configured | Self::PostLaunch { .. })
    }

    fn has_pending_launch(&self) -> bool {
        matches!(self, Self::AwaitingConfig { pending: Some(_) })
    }

    fn pending_actions(&self) -> LaunchActions {
        match self {
            Self::PostLaunch { actions } => *actions,
            _ => LaunchActions::default(),
        }
    }

    fn pending_since(&self) -> Option<Instant> {
        match self {
            Self::AwaitingConfig {
                pending: Some(pending),
            } => match pending {
                PendingStart::Launch(pending) => Some(pending.since),
                PendingStart::Attach(pending) => Some(pending.since),
            },
            _ => None,
        }
    }

    fn set_pending(&mut self, pending: PendingStart) {
        *self = Self::AwaitingConfig {
            pending: Some(pending),
        };
    }

    fn take_pending(&mut self) -> Option<PendingStart> {
        match self {
            Self::AwaitingConfig { pending } => pending.take(),
            _ => None,
        }
    }

    fn set_configured(&mut self) {
        *self = Self::Configured;
    }

    fn set_post_launch(&mut self, actions: LaunchActions) {
        *self = Self::PostLaunch { actions };
    }

    fn take_actions(&mut self) -> LaunchActions {
        match self {
            Self::PostLaunch { actions } => {
                let taken = *actions;
                *self = Self::Configured;
                taken
            }
            _ => LaunchActions::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CoordinateConverter {
    line_offset: u32,
    column_offset: u32,
}

impl CoordinateConverter {
    fn new(lines_start_at1: bool, columns_start_at1: bool) -> Self {
        Self {
            line_offset: if lines_start_at1 { 1 } else { 0 },
            column_offset: if columns_start_at1 { 1 } else { 0 },
        }
    }

    fn lines_start_at1(&self) -> bool {
        self.line_offset != 0
    }

    fn columns_start_at1(&self) -> bool {
        self.column_offset != 0
    }

    fn to_client_line(self, line: u32) -> u32 {
        line.saturating_add(self.line_offset)
    }

    fn to_client_column(self, column: u32) -> u32 {
        column.saturating_add(self.column_offset)
    }

    fn to_runtime_line(self, line: u32) -> Option<u32> {
        line.checked_sub(self.line_offset)
    }

    fn to_runtime_column(self, column: u32) -> Option<u32> {
        column.checked_sub(self.column_offset)
    }

    fn default_line(&self) -> u32 {
        self.line_offset
    }

    fn default_column(&self) -> u32 {
        self.column_offset
    }
}

/// Minimal debug adapter wrapper around a debug session.
pub struct DebugAdapter {
    session: Box<dyn DebugRuntime>,
    remote_session: Option<RemoteSession>,
    remote_stop_poller: Option<RemoteStopPoller>,
    remote_breakpoints: Arc<Mutex<HashMap<u32, u64>>>,
    next_seq: Arc<AtomicU32>,
    coordinate: CoordinateConverter,
    variable_handles: HashMap<u32, VariableHandle>,
    next_variable_ref: u32,
    watch_cache: HashMap<String, Expr>,
    runner: Option<DebugRunner>,
    control_server: Option<DebugControlServer>,
    last_io_state: Arc<Mutex<Option<IoStateEventBody>>>,
    forced_io_addresses: Arc<Mutex<HashSet<String>>>,
    launch_state: LaunchState,
    pause_expected: Arc<AtomicBool>,
    stop_gate: StopGate,
    dap_writer: Option<Arc<Mutex<BufWriter<std::io::Stdout>>>>,
    dap_logger: Option<Arc<Mutex<BufWriter<File>>>>,
}

#[derive(Debug, Default)]
struct DispatchOutcome {
    responses: Vec<Value>,
    events: Vec<Value>,
    should_exit: bool,
    stop_gate: Option<StopGateToken>,
}

#[derive(Debug, Clone)]
struct StopGate {
    inner: Arc<StopGateInner>,
}

#[derive(Debug)]
struct StopGateInner {
    count: Mutex<usize>,
    cvar: Condvar,
}

#[derive(Debug)]
struct StopGateToken {
    inner: Arc<StopGateInner>,
}

impl StopGate {
    fn new() -> Self {
        Self {
            inner: Arc::new(StopGateInner {
                count: Mutex::new(0),
                cvar: Condvar::new(),
            }),
        }
    }

    fn enter(&self) -> StopGateToken {
        let mut count = self.inner.count.lock().expect("stop gate poisoned");
        *count = count.saturating_add(1);
        StopGateToken {
            inner: Arc::clone(&self.inner),
        }
    }

    fn wait_clear(&self) {
        let mut count = self.inner.count.lock().expect("stop gate poisoned");
        while *count > 0 {
            count = self.inner.cvar.wait(count).expect("stop gate poisoned");
        }
    }
}

impl Drop for StopGateToken {
    fn drop(&mut self) {
        let mut count = self.inner.count.lock().expect("stop gate poisoned");
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.inner.cvar.notify_all();
        }
    }
}
