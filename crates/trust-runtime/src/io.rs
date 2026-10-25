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
mod loopback;
#[cfg(target_os = "linux")]
mod pwm;
mod registry;
pub use gpio::{discover_gpio_lines, GpioDriver, GpioLine, GpioLineInfo};
pub use loopback::LoopbackIoDriver;
pub use registry::IoDriverRegistry;

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
