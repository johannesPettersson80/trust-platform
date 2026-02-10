//! I/O driver registry for runtime configuration.

use std::collections::BTreeSet;
use std::collections::HashMap;

use smol_str::SmolStr;

use crate::error::RuntimeError;

use super::{
    GpioDriver, IoDriver, LoopbackIoDriver, ModbusTcpDriver, MqttIoDriver, SimulatedIoDriver,
};

pub struct IoDriverRegistry {
    entries: HashMap<SmolStr, IoDriverRegistryEntry>,
}

impl Default for IoDriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct IoDriverSpec {
    pub name: SmolStr,
    pub driver: Box<dyn IoDriver>,
}

type IoDriverCreate = fn(&toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError>;
type IoDriverValidate = fn(&toml::Value) -> Result<(), RuntimeError>;

#[derive(Clone)]
struct IoDriverRegistryEntry {
    canonical: SmolStr,
    create: IoDriverCreate,
    validate: IoDriverValidate,
}

impl IoDriverRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn default_registry() -> Self {
        let mut registry = Self::new();
        registry.register("simulated", create_simulated, validate_simulated);
        registry.register_alias("sim", "simulated");
        registry.register_alias("noop", "simulated");
        registry.register("loopback", create_loopback, validate_simulated);

        registry.register("gpio", create_gpio, validate_gpio);

        registry.register("modbus-tcp", create_modbus_tcp, validate_modbus_tcp);
        registry.register_alias("modbus_tcp", "modbus-tcp");

        registry.register("mqtt", create_mqtt, validate_mqtt);
        registry.register_alias("mqtt-tcp", "mqtt");
        registry
    }

    pub fn register(
        &mut self,
        name: impl Into<SmolStr>,
        create: IoDriverCreate,
        validate: IoDriverValidate,
    ) {
        let canonical = normalize_name(name.into());
        let entry = IoDriverRegistryEntry {
            canonical: canonical.clone(),
            create,
            validate,
        };
        self.entries.insert(canonical, entry);
    }

    pub fn register_alias(&mut self, alias: impl Into<SmolStr>, target: &str) {
        let alias = normalize_name(alias.into());
        let target = normalize_name(SmolStr::new(target));
        if let Some(entry) = self.entries.get(&target).cloned() {
            self.entries.insert(alias, entry);
        }
    }

    pub fn validate(&self, driver: &str, params: &toml::Value) -> Result<(), RuntimeError> {
        if is_none_driver(driver) {
            return Ok(());
        }
        let entry = self
            .entries
            .get(&normalize_name(SmolStr::new(driver)))
            .cloned()
            .ok_or_else(|| {
                RuntimeError::InvalidConfig(format!("unsupported io.driver '{driver}'").into())
            })?;
        (entry.validate)(params)
    }

    pub fn build(
        &self,
        driver: &str,
        params: &toml::Value,
    ) -> Result<Option<IoDriverSpec>, RuntimeError> {
        if is_none_driver(driver) {
            return Ok(None);
        }
        let entry = self
            .entries
            .get(&normalize_name(SmolStr::new(driver)))
            .cloned()
            .ok_or_else(|| {
                RuntimeError::InvalidConfig(format!("unsupported io.driver '{driver}'").into())
            })?;
        let driver = (entry.create)(params)?;
        Ok(Some(IoDriverSpec {
            name: entry.canonical,
            driver,
        }))
    }

    /// Return the canonical built-in driver names (stable sorted).
    pub fn canonical_driver_names(&self) -> Vec<String> {
        let mut names = BTreeSet::new();
        for entry in self.entries.values() {
            names.insert(entry.canonical.to_string());
        }
        names.into_iter().collect()
    }
}

fn normalize_name(name: SmolStr) -> SmolStr {
    SmolStr::new(name.as_str().trim().to_ascii_lowercase())
}

fn is_none_driver(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case("none")
}

fn validate_simulated(_params: &toml::Value) -> Result<(), RuntimeError> {
    Ok(())
}

fn create_simulated(_params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    Ok(Box::new(SimulatedIoDriver))
}

fn create_loopback(_params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    Ok(Box::new(LoopbackIoDriver::default()))
}

fn validate_gpio(params: &toml::Value) -> Result<(), RuntimeError> {
    GpioDriver::validate_params(params)?;
    Ok(())
}

fn create_gpio(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let driver = GpioDriver::from_params(params)?;
    Ok(Box::new(driver))
}

fn validate_modbus_tcp(params: &toml::Value) -> Result<(), RuntimeError> {
    let _ = ModbusTcpDriver::from_params(params)?;
    Ok(())
}

fn create_modbus_tcp(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let driver = ModbusTcpDriver::from_params(params)?;
    Ok(Box::new(driver))
}

fn validate_mqtt(params: &toml::Value) -> Result<(), RuntimeError> {
    MqttIoDriver::validate_params(params)?;
    Ok(())
}

fn create_mqtt(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let driver = MqttIoDriver::from_params(params)?;
    Ok(Box::new(driver))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_driver_names_are_sorted_unique() {
        let registry = IoDriverRegistry::default_registry();
        let names = registry.canonical_driver_names();
        assert_eq!(
            names,
            vec![
                "gpio".to_string(),
                "loopback".to_string(),
                "modbus-tcp".to_string(),
                "mqtt".to_string(),
                "simulated".to_string(),
            ]
        );
    }

    #[test]
    fn alias_resolves_to_canonical_driver_name() {
        let registry = IoDriverRegistry::default_registry();
        let spec = registry
            .build("sim", &toml::Value::Table(toml::map::Map::new()))
            .expect("build simulated alias")
            .expect("driver spec");
        assert_eq!(spec.name.as_str(), "simulated");
    }
}
