//! Conformance suite runner command.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::json;
use trust_runtime::ads::diagnostics::AdsConnectionStatusState;
use trust_runtime::ads::AdsConnectionState;
use trust_runtime::connectors::{
    ads_connection_state_status, ads_connection_status_state, ethercat_status, io_driver_status,
    modbus_status, mqtt_session_status, opcua_client_status, opcua_server_snapshot_status,
    ConnectorHealth, ConnectorState, EthercatProjection, ModbusProjection, MqttSessionProjection,
    OpcUaServerSnapshotState,
};
use trust_runtime::device_link::{self, DeviceLink};
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::{IoAddress, IoDriverErrorPolicy, IoDriverHealth};
use trust_runtime::native::{self, Target, Toolchain};
use trust_runtime::opcua::OpcUaClientConnectionState;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

use crate::cli::ConformanceArgs;

include!("conformance/models.rs");
include!("conformance/runner.rs");
include!("conformance/discovery.rs");
include!("conformance/execution.rs");
include!("conformance/native_execution.rs");
include!("conformance/device_execution.rs");
include!("conformance/series_values.rs");
include!("conformance/time_utils.rs");
include!("conformance/tests.rs");
