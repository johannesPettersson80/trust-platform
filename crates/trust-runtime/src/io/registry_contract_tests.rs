use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::*;

static REGISTRY_CONTRACT_CREATE_COUNT: AtomicUsize = AtomicUsize::new(0);
static REGISTRY_CONTRACT_VALIDATE_COUNT: AtomicUsize = AtomicUsize::new(0);
static REGISTRY_CONTRACT_COUNTER_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct RegistryContractDriver;

impl IoDriver for RegistryContractDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

fn registry_contract_create(_params: &toml::Value) -> Result<Box<dyn IoDriver>, RuntimeError> {
    REGISTRY_CONTRACT_CREATE_COUNT.fetch_add(1, Ordering::SeqCst);
    Ok(Box::new(RegistryContractDriver))
}

fn registry_contract_validate(_params: &toml::Value) -> Result<(), RuntimeError> {
    REGISTRY_CONTRACT_VALIDATE_COUNT.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn registry_contract_empty_params() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

#[test]
fn registry_contract_lookup_trims_and_folds_ascii_case() {
    let registry = IoDriverRegistry::default_registry();
    for name in ["simulated", " SIMULATED ", "Simulated", "\tsImUlAtEd\n"] {
        let spec = registry
            .build(name, &registry_contract_empty_params())
            .expect("normalized lookup")
            .expect("driver");
        assert_eq!(spec.name.as_str(), "simulated");
    }
}

#[test]
fn registry_contract_every_builtin_alias_points_to_exact_canonical_entry() {
    let registry = IoDriverRegistry::default_registry();
    for (alias, canonical) in [
        ("sim", "simulated"),
        ("noop", "simulated"),
        ("modbus_tcp", "modbus-tcp"),
        ("mqtt-tcp", "mqtt"),
        ("ether-cat", "ethercat"),
        ("ecat", "ethercat"),
    ] {
        let entry = registry
            .entries
            .get(&normalize_name(SmolStr::new(alias)))
            .unwrap_or_else(|| panic!("missing alias {alias}"));
        assert_eq!(entry.canonical.as_str(), canonical, "{alias}");
    }
}

#[test]
fn registry_contract_none_is_trimmed_case_insensitive_and_parameter_independent() {
    let registry = IoDriverRegistry::default_registry();
    let arbitrary = toml::toml! {
        invalid = ["parameters", "are", "not", "read"]
    }
    .into();
    for name in ["none", " NONE ", "NoNe"] {
        registry.validate(name, &arbitrary).expect("none validates");
        assert!(registry
            .build(name, &arbitrary)
            .expect("none builds")
            .is_none());
    }
}

#[test]
fn registry_contract_empty_and_unknown_names_fail_validation_and_build() {
    let registry = IoDriverRegistry::default_registry();
    for name in ["", " \t ", "missing-driver"] {
        let validate = registry
            .validate(name, &registry_contract_empty_params())
            .expect_err("invalid driver validation");
        let build = match registry.build(name, &registry_contract_empty_params()) {
            Ok(_) => panic!("invalid driver build succeeded"),
            Err(error) => error,
        };
        assert!(validate.to_string().contains("unsupported io.driver"));
        assert!(build.to_string().contains("unsupported io.driver"));
    }
}

#[test]
fn registry_contract_validation_never_invokes_constructor() {
    let _guard = REGISTRY_CONTRACT_COUNTER_LOCK
        .lock()
        .expect("lock registry counters");
    REGISTRY_CONTRACT_CREATE_COUNT.store(0, Ordering::SeqCst);
    REGISTRY_CONTRACT_VALIDATE_COUNT.store(0, Ordering::SeqCst);
    let mut registry = IoDriverRegistry::new();
    registry.register(
        "counting",
        registry_contract_create,
        registry_contract_validate,
    );

    registry
        .validate("counting", &registry_contract_empty_params())
        .expect("validate custom driver");

    assert_eq!(REGISTRY_CONTRACT_VALIDATE_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(REGISTRY_CONTRACT_CREATE_COUNT.load(Ordering::SeqCst), 0);
}

#[test]
fn registry_contract_build_validates_then_constructs_and_returns_canonical_name() {
    let _guard = REGISTRY_CONTRACT_COUNTER_LOCK
        .lock()
        .expect("lock registry counters");
    REGISTRY_CONTRACT_CREATE_COUNT.store(0, Ordering::SeqCst);
    REGISTRY_CONTRACT_VALIDATE_COUNT.store(0, Ordering::SeqCst);
    let mut registry = IoDriverRegistry::new();
    registry.register(
        " Counting ",
        registry_contract_create,
        registry_contract_validate,
    );
    registry.register_alias("counter", "counting");

    let spec = registry
        .build(" COUNTER ", &registry_contract_empty_params())
        .expect("build custom alias")
        .expect("driver");

    assert_eq!(spec.name.as_str(), "counting");
    assert_eq!(REGISTRY_CONTRACT_CREATE_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(REGISTRY_CONTRACT_VALIDATE_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn registry_contract_alias_cannot_shadow_canonical_driver() {
    let mut registry = IoDriverRegistry::default_registry();
    registry.register_alias("simulated", "mqtt");

    let entry = registry
        .entries
        .get("simulated")
        .expect("canonical simulated entry");
    assert_eq!(entry.canonical.as_str(), "simulated");
}

#[test]
fn registry_contract_missing_alias_target_never_creates_entry() {
    let mut registry = IoDriverRegistry::default_registry();
    registry.register_alias("ghost", "missing-target");

    assert!(!registry.entries.contains_key("ghost"));
    assert!(registry
        .validate("ghost", &registry_contract_empty_params())
        .is_err());
}

#[test]
fn registry_contract_empty_and_none_registration_do_not_enter_enumeration() {
    let mut registry = IoDriverRegistry::new();
    registry.register("", registry_contract_create, registry_contract_validate);
    registry.register(
        " none ",
        registry_contract_create,
        registry_contract_validate,
    );
    registry.register(
        "valid",
        registry_contract_create,
        registry_contract_validate,
    );

    assert_eq!(registry.canonical_driver_names(), vec!["valid".to_string()]);
    assert!(registry
        .build("none", &registry_contract_empty_params())
        .expect("reserved none")
        .is_none());
}

#[test]
fn registry_contract_canonical_enumeration_excludes_all_aliases() {
    let registry = IoDriverRegistry::default_registry();
    let names = registry.canonical_driver_names();
    for alias in ["sim", "noop", "modbus_tcp", "mqtt-tcp", "ether-cat", "ecat"] {
        assert!(!names.iter().any(|name| name == alias), "{alias}");
    }
    assert!(!names.iter().any(|name| name == "none"));
    assert!(!names.iter().any(String::is_empty));
}
