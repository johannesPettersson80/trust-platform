//! Selected process-image engineering writes use the same core window/codec policy.
use trust_runtime_core::{
    bytecode::{BytecodeModule, IoBinding, SectionData, SectionId},
    error::RuntimeError,
    io_address::IoAddress,
    value::{Duration, Value},
    vm::{PreparationLimits, PreparedModule},
};

const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

fn hierarchy_module() -> BytecodeModule {
    let mut module = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::IoMap(io)) = module.section(SectionId::IoMap) else {
        panic!("I/O metadata")
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let input = io
        .bindings
        .iter()
        .find(|binding| strings.entries[binding.address_str_idx as usize].starts_with("%I"))
        .unwrap()
        .clone();
    let output = io
        .bindings
        .iter()
        .find(|binding| strings.entries[binding.address_str_idx as usize].starts_with("%Q"))
        .unwrap()
        .clone();
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let first = strings.entries.len() as u32;
    for name in ["%IX3.4.0", "%QX3.4.0", "%MX3.4.0"] {
        strings.entries.push(name.into());
    }
    let Some(SectionData::IoMap(io)) = module.section_mut(SectionId::IoMap) else {
        panic!("I/O metadata")
    };
    io.bindings.push(IoBinding {
        address_str_idx: first,
        ..input
    });
    io.bindings.push(IoBinding {
        address_str_idx: first + 1,
        ..output.clone()
    });
    io.bindings.push(IoBinding {
        address_str_idx: first + 2,
        ..output
    });
    module
}

macro_rules! flat_checks {
    ($state:expr) => {{
        let state = &mut $state;
        let input = IoAddress::parse("%IX0.0").unwrap();
        let output = IoAddress::parse("%QX0.0").unwrap();
        state.write_image(&input, Value::Bool(true)).unwrap();
        assert_eq!(state.inputs(), &[1]);
        state.write_image(&output, Value::Bool(true)).unwrap();
        assert_eq!(state.read_image(&output).unwrap(), Value::Bool(true));
        let deadline = state.task_states()[0].last_run;
        for (address, value) in [
            (output.clone(), Value::String("invalid bit value".into())),
            (IoAddress::parse("%QB1").unwrap(), Value::Byte(0)),
            (IoAddress::parse("%QX3.4.0").unwrap(), Value::Bool(true)),
        ] {
            assert!(state.write_image(&address, value).is_err());
            assert_eq!(state.inputs(), &[1]);
            assert_eq!(state.outputs(), &[1]);
            assert_eq!(state.task_states()[0].last_run, deadline);
            assert!(state.fault().is_none());
        }
        state.write_image(&output, Value::Bool(false)).unwrap();
        state.execute_cycle(Duration::from_millis(10)).unwrap();
        assert_eq!(
            state.outputs(),
            &[1],
            "the cycle samples mapped bindings from the same inputs"
        );
    }};
}

#[test]
fn borrowed_image_writes_are_selected_checked_operations_without_scan_or_fault_side_effects() {
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    flat_checks!(state);
}

#[test]
#[cfg(feature = "std")]
fn owned_image_writes_use_the_identical_window_and_codec_policy() {
    let prepared = std::sync::Arc::new(
        PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap(),
    );
    let mut state = prepared.instantiate_owned(0).unwrap();
    flat_checks!(state);
}

#[test]
fn borrowed_hierarchy_projection_includes_admitted_inputs_outputs_and_markers_only() {
    let module = hierarchy_module();
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for text in ["%IX3.4.0", "%QX3.4.0", "%MX3.4.0"] {
        let address = IoAddress::parse(text).unwrap();
        state.write_image(&address, Value::Bool(true)).unwrap();
        assert_eq!(
            state.read_hierarchical_image_ref(&address),
            Some(&Value::Bool(true))
        );
        assert_eq!(state.read_image(&address).unwrap(), Value::Bool(true));
        assert!(state
            .write_image(&address, Value::String("bad wire type".into()))
            .is_err());
        assert_eq!(
            state.read_hierarchical_image_ref(&address),
            Some(&Value::Bool(true))
        );
    }
    let unbound = IoAddress::parse("%IX3.5.0").unwrap();
    assert!(state.read_hierarchical_image_ref(&unbound).is_none());
    assert!(state.write_image(&unbound, Value::Bool(true)).is_err());
    assert_eq!(
        state.read_hierarchical_output(&IoAddress::parse("%IX3.4.0").unwrap()),
        None,
        "the old output-only accessor does not silently acquire input semantics"
    );
    assert_eq!(state.task_states()[0].last_run, Duration::ZERO);
    assert!(state.fault().is_none());
}

#[test]
#[cfg(feature = "std")]
fn owned_hierarchy_projection_borrows_the_same_selected_value_as_engineering_read() {
    let prepared = std::sync::Arc::new(
        PreparedModule::from_decoded(&hierarchy_module(), PreparationLimits::default()).unwrap(),
    );
    let mut state = prepared.instantiate_owned(0).unwrap();
    let input = IoAddress::parse("%IX3.4.0").unwrap();
    state.write_image(&input, Value::Bool(true)).unwrap();
    assert_eq!(
        state.read_hierarchical_image_ref(&input),
        Some(&Value::Bool(true))
    );
    assert_eq!(state.read_image(&input).unwrap(), Value::Bool(true));
}

#[test]
fn expired_engineering_image_write_rejects_before_mutation_without_latching_a_plc_fault() {
    struct Services(std::cell::Cell<bool>);
    impl trust_runtime_core::vm::ExecutionServices for Services {
        fn deadline_exceeded(&self) -> bool {
            self.0.get()
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<trust_runtime_core::value::DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
    }
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let services = Services(std::cell::Cell::new(false));
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    services.0.set(true);
    assert_eq!(
        state.write_image(&IoAddress::parse("%QX0.0").unwrap(), Value::Bool(true)),
        Err(RuntimeError::ExecutionTimeout)
    );
    assert_eq!(state.outputs(), &[0]);
    assert_eq!(state.task_states()[0].last_run, Duration::ZERO);
    assert!(state.fault().is_none());
}
