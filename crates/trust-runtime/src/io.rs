//! Direct I/O mapping and images.

#![allow(missing_docs)]

use smol_str::SmolStr;

mod modbus;
pub use modbus::ModbusTcpDriver;
mod mqtt;
pub use mqtt::MqttIoDriver;
mod mqtt_tag_mapping;
pub use mqtt_tag_mapping::{resolve_compiled_mqtt_tag_mappings, MqttTagLink};
mod ethercat;
pub use ethercat::{
    configured_ethercat_modules, discover_ethercat_modules, EthercatDiscoveryInfo,
    EthercatIoDriver, EthercatModuleInfo,
};
// The drivers that every runtime shares (specification 11 §6.5) run on Linux; elsewhere the
// registry refuses them when they start.
#[cfg(target_os = "linux")]
mod analog;
#[cfg(target_os = "linux")]
mod can;
#[cfg(target_os = "linux")]
mod counter;
#[cfg(all(test, target_os = "linux"))]
mod fake_sysfs;
mod gpio;
mod identify;
mod link_names;
mod loopback;
#[cfg(target_os = "linux")]
mod pwm;
mod registry;
pub use gpio::{discover_gpio_lines, GpioDriver, GpioIdentifyLine, GpioLine, GpioLineInfo};
pub(crate) use identify::IdleOutput;
pub use identify::{
    check_identify_output, check_identify_replacement, check_runtime_identify_output,
    host_identify, safe_outputs_hold_bit, safe_state_holds_bit, HostIdentify, HostIdentifyOutput,
    IdentifyHandle, ImageBit,
};
pub use link_names::{io_link_name, link_protocol};
pub use loopback::LoopbackIoDriver;
pub use registry::{is_nonphysical_driver, IoDriverRegistry};

use crate::error::RuntimeError;
use crate::memory::IoArea;
use crate::memory::VariableStorage;
use crate::value::Value;
use crate::value::ValueRef;
use trust_hir::TypeId;

include!("io/driver.rs");
include!("io/addressing.rs");
include!("io/coercion.rs");
include!("io/provenance.rs");
include!("io/interface.rs");

#[cfg(test)]
#[path = "io/provenance_contract_tests.rs"]
mod provenance_contract_tests;
