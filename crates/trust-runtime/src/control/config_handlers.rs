use std::sync::atomic::Ordering;

use crate::config::{
    ControlMode, RuntimeCloudLinkPreferenceRule, RuntimeCloudPreferredTransport,
    RuntimeCloudProfile, RuntimeCloudWanAllowRule,
};
use serde_json::json;
use smol_str::SmolStr;

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
