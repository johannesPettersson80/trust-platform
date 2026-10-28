use std::sync::atomic::Ordering;

use crate::config::{
    ControlMode, RuntimeCloudLinkPreferenceRule, RuntimeCloudPreferredTransport,
    RuntimeCloudProfile, RuntimeCloudWanAllowRule,
};
use crate::settings::RuntimeSettings;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::deployment::{
    ControlMode as SharedControlMode, HangAction as SharedHangAction, LogLevel,
};
use trust_runtime_services::ports::StatusPort;
use trust_runtime_services::settings::values::{
    config_type_error, config_value_error, expect_bool, expect_non_empty_string,
    expect_string_array, expect_string_map,
};
use trust_runtime_services::settings::{
    self as shared, RuntimeSettings as SharedSettings, StagedSettings,
};

use super::{ControlResponse, ControlState};

include!("config_handlers_get.rs");
include!("config_handlers_set.rs");
include!("config_handlers_validate.rs");

#[cfg(test)]
#[path = "config_handlers/handler_contract_tests.rs"]
mod handler_contract_tests;
#[cfg(test)]
#[path = "config_handlers/validation_contract_tests.rs"]
mod validation_contract_tests;
