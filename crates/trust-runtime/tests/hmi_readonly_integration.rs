use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::config::{ControlMode, WebAuthMode, WebConfig};
use trust_runtime::control::{ControlState, HmiRuntimeDescriptor, SourceRegistry};
use trust_runtime::debug::DebugVariableHandles;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::web::start_web_server;

include!("../../../tests/support/repository_source_oracle.rs");

#[path = "hmi_readonly_integration/hmi_readonly_integration_support_01.rs"]
mod hmi_readonly_integration_support_01;
#[path = "hmi_readonly_integration/hmi_readonly_integration_support_02.rs"]
mod hmi_readonly_integration_support_02;
#[path = "hmi_readonly_integration/hmi_readonly_integration_support_03.rs"]
mod hmi_readonly_integration_support_03;
use hmi_readonly_integration_support_01::*;
use hmi_readonly_integration_support_02::*;
use hmi_readonly_integration_support_03::*;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_01.rs"]
mod hmi_readonly_integration_part_01;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_02.rs"]
mod hmi_readonly_integration_part_02;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_03.rs"]
mod hmi_readonly_integration_part_03;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_04.rs"]
mod hmi_readonly_integration_part_04;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_05.rs"]
mod hmi_readonly_integration_part_05;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_06.rs"]
mod hmi_readonly_integration_part_06;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_07.rs"]
mod hmi_readonly_integration_part_07;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_08.rs"]
mod hmi_readonly_integration_part_08;
#[path = "hmi_readonly_integration/hmi_readonly_integration_part_09.rs"]
mod hmi_readonly_integration_part_09;
