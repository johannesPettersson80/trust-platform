use std::collections::BTreeMap;
use std::path::Path;

use crate::scheduler::ResourceCommand;
use crate::value::Value;
use notify::{Event, EventKind};
use serde_json::json;
use smol_str::SmolStr;

use super::types::{
    HmiAlarmAckParams, HmiAlarmsParams, HmiDescriptorUpdateParams, HmiScaffoldResetParams,
    HmiTrendsParams, HmiValuesParams, HmiWriteParams,
};
use super::{ControlResponse, ControlState, HmiRuntimeDescriptor, SourceRegistry};

include!("hmi_runtime_ports.rs");
include!("hmi_handlers_read.rs");
include!("hmi_handlers_descriptor.rs");
include!("hmi_handlers_write.rs");
include!("hmi_handlers_state.rs");
include!("hmi_handlers_parse.rs");
