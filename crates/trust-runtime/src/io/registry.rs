//! I/O driver registry for runtime configuration.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fmt::Display;

use smol_str::SmolStr;
use trust_runtime_services::io_drivers::analog::AnalogParams;
use trust_runtime_services::io_drivers::can::CanParams;
use trust_runtime_services::io_drivers::counter::CounterParams;
use trust_runtime_services::io_drivers::pwm::PwmParams;
use trust_runtime_services::io_drivers::ParamsError;

use crate::error::RuntimeError;

#[cfg(target_os = "linux")]
use super::{analog::AnalogDriver, can::CanDriver, counter::CounterDriver, pwm::PwmDriver};
use super::{
    EthercatIoDriver, GpioDriver, IoDriver, LoopbackIoDriver, ModbusTcpDriver, MqttIoDriver,
    SimulatedIoDriver,
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

        registry.register("ethercat", create_ethercat, validate_ethercat);
        registry.register_alias("ether-cat", "ethercat");
        registry.register_alias("ecat", "ethercat");

        // The drivers every runtime shares (specification 11 §6.5), with the parameter rules of
        // `trust_runtime_services::io_drivers`.
        registry.register("analog", create_analog, validate_analog);
        registry.register("can", create_can, validate_can);
        registry.register("counter", create_counter, validate_counter);
        registry.register("pwm", create_pwm, validate_pwm);
        registry
    }

    pub fn register(
        &mut self,
        name: impl Into<SmolStr>,
        create: IoDriverCreate,
        validate: IoDriverValidate,
    ) {
        let canonical = normalize_name(name.into());
        if canonical.is_empty() || is_none_driver(canonical.as_str()) {
            return;
        }
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
        if alias.is_empty() || is_none_driver(alias.as_str()) || self.entries.contains_key(&alias) {
            return;
        }
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
        (entry.validate)(params)?;
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

/// Reads the parameters by the shared rules without constructing the driver, so that validation
/// opens no connection and starts no worker (specification 11 §6.5).
fn validate_modbus_tcp(params: &toml::Value) -> Result<(), RuntimeError> {
    ModbusTcpDriver::validate_params(params)
}

fn create_modbus_tcp(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let driver = ModbusTcpDriver::from_params(params)?;
    Ok(Box::new(driver))
}

/// Reads the parameters by the shared rules and the TLS files that they name, without constructing
/// the driver, so that validation opens no connection and starts no worker (specification 11
/// §6.5).
fn validate_mqtt(params: &toml::Value) -> Result<(), RuntimeError> {
    MqttIoDriver::validate_params(params)
}

fn create_mqtt(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let driver = MqttIoDriver::from_params(params)?;
    Ok(Box::new(driver))
}

fn validate_ethercat(params: &toml::Value) -> Result<(), RuntimeError> {
    EthercatIoDriver::validate_params(params)?;
    Ok(())
}

fn create_ethercat(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let driver = EthercatIoDriver::from_params(params)?;
    Ok(Box::new(driver))
}

fn validate_analog(params: &toml::Value) -> Result<(), RuntimeError> {
    shared_params("analog", params, AnalogParams::from_json)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn create_analog(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let params = shared_params("analog", params, AnalogParams::from_json)?;
    Ok(Box::new(AnalogDriver::start(params)?))
}

#[cfg(not(target_os = "linux"))]
fn create_analog(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    validate_analog(params)?;
    Err(unavailable(
        "analog",
        "analog I/O is not available on this computer",
    ))
}

fn validate_can(params: &toml::Value) -> Result<(), RuntimeError> {
    shared_params("can", params, CanParams::from_json)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn create_can(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let params = shared_params("can", params, CanParams::from_json)?;
    Ok(Box::new(CanDriver::start(params)?))
}

#[cfg(not(target_os = "linux"))]
fn create_can(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    validate_can(params)?;
    Err(unavailable("can", "CAN is not available on this computer"))
}

fn validate_counter(params: &toml::Value) -> Result<(), RuntimeError> {
    shared_params("counter", params, CounterParams::from_json)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn create_counter(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let params = shared_params("counter", params, CounterParams::from_json)?;
    Ok(Box::new(CounterDriver::start(params)?))
}

#[cfg(not(target_os = "linux"))]
fn create_counter(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    validate_counter(params)?;
    Err(unavailable(
        "counter",
        "hardware counters are not available on this computer",
    ))
}

fn validate_pwm(params: &toml::Value) -> Result<(), RuntimeError> {
    shared_params("pwm", params, PwmParams::from_json)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn create_pwm(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    let params = shared_params("pwm", params, PwmParams::from_json)?;
    Ok(Box::new(PwmDriver::start(params)?))
}

#[cfg(not(target_os = "linux"))]
fn create_pwm(params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    validate_pwm(params)?;
    Err(unavailable("pwm", "PWM is not available on this computer"))
}

/// The refusal, on a computer that is not Linux, of a driver whose hardware the computer lacks,
/// when the driver starts (specification 11 §6.5, specification 34 ARCH-13).
#[cfg(not(target_os = "linux"))]
fn unavailable(driver: &str, reason: &str) -> RuntimeError {
    RuntimeError::InvalidConfig(format!("{driver}: {reason}").into())
}

/// The parameters of the driver `driver`, read from `params` by its shared parser `parse`
/// (specification 11 §6.5). The table is passed as the JSON text that a chip reads from its
/// deployment record (specification 34 §5.1.1, section 8), so that a computer and a chip accept
/// and refuse the same `io.toml`. Reading opens no device. A refusal is `<driver>: <key>:
/// <reason>`. The `modbus-tcp` driver reads its parameters with it too.
pub(super) fn shared_params<P>(
    driver: &str,
    params: &toml::Value,
    parse: fn(&str) -> Result<P, ParamsError>,
) -> Result<P, RuntimeError> {
    let json = params_json(params, "").map_err(|reason| params_refused(driver, &reason))?;
    let text = serde_json::to_string(&json).map_err(|error| params_refused(driver, &error))?;
    parse(&text).map_err(|error| params_refused(driver, &error))
}

/// `value`, the table at `key` or a value in it, as JSON, as the deployment record carries a
/// driver's parameters (specification 34 §5.1.1): a date or a time as its RFC 3339 text, and a
/// float that is not finite refused with its key.
fn params_json(value: &toml::Value, key: &str) -> Result<serde_json::Value, String> {
    Ok(match value {
        toml::Value::String(text) => serde_json::Value::String(text.clone()),
        toml::Value::Integer(number) => serde_json::Value::from(*number),
        toml::Value::Float(number) => serde_json::Number::from_f64(*number)
            .map(serde_json::Value::Number)
            .ok_or_else(|| format!("{key}: {number} is not a finite number"))?,
        toml::Value::Boolean(flag) => serde_json::Value::Bool(*flag),
        toml::Value::Datetime(when) => serde_json::Value::String(when.to_string()),
        toml::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| params_json(item, &format!("{key}[{index}]")))
                .collect::<Result<_, _>>()?,
        ),
        toml::Value::Table(table) => serde_json::Value::Object(
            table
                .iter()
                .map(|(name, item)| {
                    let path = if key.is_empty() {
                        name.clone()
                    } else {
                        format!("{key}.{name}")
                    };
                    params_json(item, &path).map(|item| (name.clone(), item))
                })
                .collect::<Result<_, _>>()?,
        ),
    })
}

/// The refusal of the parameters of the driver `driver`: its name and `reason`.
fn params_refused(driver: &str, reason: &dyn Display) -> RuntimeError {
    RuntimeError::InvalidConfig(format!("{driver}: {reason}").into())
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
                "analog".to_string(),
                "can".to_string(),
                "counter".to_string(),
                "ethercat".to_string(),
                "gpio".to_string(),
                "loopback".to_string(),
                "modbus-tcp".to_string(),
                "mqtt".to_string(),
                "pwm".to_string(),
                "simulated".to_string(),
            ]
        );
    }

    /// The parameters of one `pwm` output on `line` at `frequency_hz`.
    fn pwm_params(line: &str, frequency_hz: u32) -> toml::Value {
        toml::from_str(&format!(
            "outputs = [{{ address = \"%QW0\", line = \"{line}\", frequency_hz = {frequency_hz} }}]"
        ))
        .expect("pwm parameters")
    }

    /// Each shared driver with parameters that name hardware no computer has, and its refusal
    /// when it starts on Linux.
    fn drivers_without_hardware() -> [(&'static str, toml::Value, &'static str); 4] {
        let params = |text: &str| -> toml::Value { toml::from_str(text).expect("parameters") };
        [
            (
                "analog",
                params(r#"inputs = [{ address = "%IW0", line = "iio:device999/in_voltage0" }]"#),
                "analog: this computer has no IIO device iio:device999",
            ),
            (
                "can",
                params("line = \"trustcan999\"\nbitrate = 500000\n"),
                "can: this computer has no CAN interface trustcan999",
            ),
            (
                "counter",
                params(r#"inputs = [{ address = "%ID0", line = "counter999/count0" }]"#),
                "counter: this computer has no counter device counter999",
            ),
            (
                "pwm",
                pwm_params("pwmchip999/pwm0", 1000),
                "pwm: this computer has no PWM chip pwmchip999",
            ),
        ]
    }

    /// Specification 11 §6.5: the shared drivers read their parameters with the shared rules, and
    /// a refusal is `<driver>: <key>: <reason>`. A `frequency_hz` of 0 is outside 1 to 1,000,000.
    /// Catches a refusal that does not say which driver or which key, and a parameter that only
    /// fails when the driver starts.
    #[test]
    fn a_shared_driver_refuses_a_parameter_with_its_name_and_the_key() {
        let error = IoDriverRegistry::default_registry()
            .validate("pwm", &pwm_params("pwmchip0/pwm0", 0))
            .expect_err("a frequency of 0 is refused");

        assert!(
            error
                .to_string()
                .starts_with("invalid config 'pwm: outputs[0].frequency_hz: "),
            "{error}"
        );
    }

    /// Specification 11 §6.5 (registry validation and construction are separate boundaries):
    /// validation reads the parameters and opens no device, so a project validates on a
    /// computer without the hardware it names, while the driver is refused when it starts, with
    /// its name and the reason. Catches a validation that looks for the hardware, and a driver
    /// that starts over hardware that is not there.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_shared_driver_validates_without_its_hardware_and_is_refused_when_it_starts() {
        let registry = IoDriverRegistry::default_registry();
        for (driver, params, refusal) in drivers_without_hardware() {
            registry
                .validate(driver, &params)
                .unwrap_or_else(|error| panic!("{driver} must validate: {error}"));
            let error = match registry.build(driver, &params) {
                Ok(_) => panic!("{driver} must be refused"),
                Err(error) => error,
            };
            assert_eq!(error.to_string(), format!("invalid config '{refusal}'"));
        }
    }

    /// Specification 11 §6.5 and 34 ARCH-13: a runtime without the hardware, macOS and Windows
    /// among them, refuses these drivers when they start, with the driver's name and the reason,
    /// while their parameters validate. Catches a driver that starts and does nothing.
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn the_shared_drivers_are_refused_on_a_computer_without_their_hardware() {
        let registry = IoDriverRegistry::default_registry();
        for (driver, params, _) in drivers_without_hardware() {
            registry
                .validate(driver, &params)
                .unwrap_or_else(|error| panic!("{driver} must validate: {error}"));
            let error = match registry.build(driver, &params) {
                Ok(_) => panic!("{driver} must be refused"),
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .starts_with(&format!("invalid config '{driver}: "))
                    && error
                        .to_string()
                        .ends_with("not available on this computer'"),
                "{error}"
            );
        }
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

#[cfg(test)]
#[path = "registry_contract_tests.rs"]
mod contract_tests;
