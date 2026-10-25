//! Adapter core loop + request dispatch.
//! - DebugAdapter::new/session accessors
//! - run/run_with_stdio: protocol loop
//! - dispatch_request/handle_request: route DAP requests
//! - event helpers: output/stopped/terminated

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{self, BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;

use crate::protocol::{
    Breakpoint, BreakpointEventBody, Event, MessageType, Request, Response,
    SetBreakpointsArguments, SetBreakpointsResponseBody, SourceBreakpoint, StoppedEventBody,
};
use crate::runtime::DebugRuntime;

use super::protocol_io::{read_message, write_message_locked, write_protocol_log};
use super::remote::RemoteStop;
use super::util::env_flag;
use super::{CoordinateConverter, DebugAdapter, DispatchOutcome, LaunchState, StopGate};

/// The error of a request that needs a running program before a launch or an attach connected to
/// one.
pub(super) const NOT_CONNECTED: &str = "no program runs: launch or attach first";

include!("core_impl_part_01.rs");
include!("core_impl_part_02.rs");
include!("core_impl_part_03.rs");
include!("core_impl_part_04.rs");
