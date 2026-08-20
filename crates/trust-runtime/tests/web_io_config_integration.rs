use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime::config::{
    ControlMode, RuntimeCloudProfile, RuntimeCloudWanAllowRule, WebAuthMode, WebConfig,
};
use trust_runtime::control::{
    ControlAuditEvent, ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry,
};
use trust_runtime::debug::DebugVariableHandles;
use trust_runtime::discovery::{DiscoveryEntry, DiscoveryState};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::security::AccessRole;
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::web::pairing::PairingStore;
use trust_runtime::web::start_web_server;

#[path = "web_io_config_integration/web_io_config_integration_support_01.rs"]
mod web_io_config_integration_support_01;
#[path = "web_io_config_integration/web_io_config_integration_support_02.rs"]
mod web_io_config_integration_support_02;
use web_io_config_integration_support_01::*;
use web_io_config_integration_support_02::*;
#[path = "web_io_config_integration/web_io_config_integration_part_01.rs"]
mod web_io_config_integration_part_01;
#[path = "web_io_config_integration/web_io_config_integration_part_02.rs"]
mod web_io_config_integration_part_02;
#[path = "web_io_config_integration/web_io_config_integration_part_03.rs"]
mod web_io_config_integration_part_03;
#[path = "web_io_config_integration/web_io_config_integration_part_08.rs"]
mod web_io_config_integration_part_08;
#[path = "web_io_config_integration/web_io_config_integration_part_09.rs"]
mod web_io_config_integration_part_09;
#[path = "web_io_config_integration/web_io_config_integration_part_10.rs"]
mod web_io_config_integration_part_10;
#[path = "web_io_config_integration/web_io_config_integration_part_11.rs"]
mod web_io_config_integration_part_11;
#[path = "web_io_config_integration/web_io_config_integration_part_12.rs"]
mod web_io_config_integration_part_12;
#[path = "web_io_config_integration/web_io_config_integration_part_13.rs"]
mod web_io_config_integration_part_13;
#[path = "web_io_config_integration/web_io_config_integration_part_14.rs"]
mod web_io_config_integration_part_14;
#[path = "web_io_config_integration/web_io_config_integration_part_15.rs"]
mod web_io_config_integration_part_15;
#[path = "web_io_config_integration/web_io_config_integration_part_16.rs"]
mod web_io_config_integration_part_16;
#[path = "web_io_config_integration/web_io_config_integration_part_17.rs"]
mod web_io_config_integration_part_17;
#[path = "web_io_config_integration/web_io_config_integration_part_18.rs"]
mod web_io_config_integration_part_18;
#[path = "web_io_config_integration/web_io_config_integration_part_19.rs"]
mod web_io_config_integration_part_19;
#[path = "web_io_config_integration/web_io_config_integration_part_20.rs"]
mod web_io_config_integration_part_20;
#[path = "web_io_config_integration/web_io_config_integration_part_21.rs"]
mod web_io_config_integration_part_21;
#[path = "web_io_config_integration/web_io_config_integration_part_22.rs"]
mod web_io_config_integration_part_22;
