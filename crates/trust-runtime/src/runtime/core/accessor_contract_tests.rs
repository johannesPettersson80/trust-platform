use super::*;

use std::sync::{Arc, Mutex};

use crate::ads::diagnostics::{AdsStatusOverall, TargetIdentity};
use crate::execution_backend::ExecutionBackend;
use crate::io::{IoDriverHealth, IoDriverStatus};
use crate::memory::MemoryLocation;
use crate::metrics::RuntimeMetrics;
use crate::program_model::{
    ClassDef, Expr, FunctionBlockDef, FunctionDef, InterfaceDef, MethodDef,
};
use crate::task::ProgramDef;
use crate::value::{PartialAccess, RefPath, ValueRef};
use trust_hir::{Type, TypeId};

#[test]
fn accessors_resource_identity_preserves_configured_spelling() {
    let mut runtime = Runtime::new();

    runtime.set_resource_name("Line_A".into());
    assert_eq!(runtime.resource_name(), "Line_A");

    runtime.set_resource_name("resource-ß".into());
    assert_eq!(runtime.resource_name(), "resource-ß");
}

#[test]
fn accessors_storage_registry_and_initializer_views_share_live_state() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("Counter", Value::DInt(7));
    let user_type = runtime.registry_mut().register(
        "BatchCount",
        Type::Alias {
            name: "BatchCount".into(),
            target: TypeId::DINT,
        },
    );

    let initializer_id = {
        let (registry, initializers) = runtime.registry_and_initializer_catalog_mut();
        assert_eq!(registry.lookup("batchcount"), Some(user_type));
        let id = initializers.insert(Expr::Literal(Value::DInt(41)));
        initializers.set_type_default(user_type, id);
        id
    };

    assert_eq!(
        runtime.storage().get_global("Counter"),
        Some(&Value::DInt(7))
    );
    assert_eq!(runtime.registry().lookup("BATCHCOUNT"), Some(user_type));
    assert_eq!(
        runtime.initializer_catalog().type_default(user_type),
        Some(initializer_id)
    );
    assert!(matches!(
        runtime.initializer_catalog().initializer(initializer_id),
        Some(Expr::Literal(Value::DInt(41)))
    ));
}

#[test]
fn accessors_declaration_maps_preserve_values_and_replace_case_only_keys() {
    let mut runtime = Runtime::new();
    let builtin_function_blocks = runtime.function_blocks().len();

    runtime.register_function(function("Mix"));
    runtime.register_function(function("mIX"));
    runtime.register_function_block(function_block("Latch", &[], &[]));
    runtime.register_function_block(function_block("lATCH", &[], &[]));
    runtime.register_class(class("Cell", &[], &[]));
    runtime.register_class(class("cELL", &[], &[]));
    runtime.register_interface(interface("Drive"));
    runtime.register_interface(interface("dRIVE"));
    runtime.register_global_meta(
        "GlobalCount".into(),
        TypeId::DINT,
        RetainPolicy::Unspecified,
        GlobalInitValue::Value(Value::DInt(3)),
    );
    runtime.register_program(program("Main", &[])).unwrap();

    assert_eq!(runtime.functions().len(), 1);
    assert_eq!(runtime.functions()["MIX"].name, "mIX");
    assert_eq!(runtime.function_blocks().len(), builtin_function_blocks + 1);
    assert_eq!(runtime.function_blocks()["LATCH"].name, "lATCH");
    assert_eq!(runtime.classes().len(), 1);
    assert_eq!(runtime.classes()["CELL"].name, "cELL");
    assert_eq!(runtime.interfaces().len(), 1);
    assert_eq!(runtime.interfaces()["DRIVE"].name, "dRIVE");
    assert_eq!(runtime.programs()["Main"].name, "Main");
    assert!(runtime.globals().contains_key("GlobalCount"));
    assert!(runtime.stdlib().get("ABS").is_some());
}

#[test]
fn accessors_instance_initialization_context_uses_runtime_owned_components() {
    let mut runtime = Runtime::new();
    runtime.register_function(function("Prepare"));
    runtime.register_function_block(function_block("Valve", &[], &[]));
    runtime.register_class(class("Axis", &[], &[]));

    {
        let (storage, registry, classes, function_blocks, functions, stdlib) =
            runtime.instance_init_context();
        assert_eq!(registry.lookup("DINT"), Some(TypeId::DINT));
        assert!(classes.contains_key("AXIS"));
        assert!(function_blocks.contains_key("VALVE"));
        assert!(functions.contains_key("PREPARE"));
        assert!(stdlib.get("ABS").is_some());
        storage.set_global("Initialized", Value::Bool(true));
    }

    assert_eq!(
        runtime.storage().get_global("Initialized"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn accessors_execution_backend_update_is_reflected_in_metrics_sink() {
    let mut runtime = Runtime::new();
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::new()));
    runtime.set_metrics_sink(metrics.clone());

    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .unwrap();

    assert_eq!(runtime.execution_backend(), ExecutionBackend::BytecodeVm);
    assert_eq!(
        metrics.lock().unwrap().snapshot().execution_backend,
        ExecutionBackend::BytecodeVm
    );
}

#[test]
fn accessors_vm_register_profile_toggle_and_reset_preserve_enablement() {
    let mut runtime = Runtime::new();
    assert!(!runtime.vm_register_profile_snapshot().enabled);

    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();
    let enabled = runtime.vm_register_profile_snapshot();
    assert!(enabled.enabled);
    assert_eq!(enabled.register_programs_executed, 0);
    assert_eq!(enabled.register_program_fallbacks, 0);
    assert!(enabled.fallback_reasons.is_empty());
    assert!(enabled.hot_blocks.is_empty());

    runtime.set_vm_register_profile_enabled(false);
    assert!(!runtime.vm_register_profile_snapshot().enabled);
}

#[test]
fn accessors_vm_lowering_cache_toggle_and_reset_preserve_configuration() {
    let mut runtime = Runtime::new();
    let initial = runtime.vm_register_lowering_cache_snapshot();
    assert!(initial.cache_capacity > 0);

    runtime.set_vm_register_lowering_cache_enabled(false);
    runtime.reset_vm_register_lowering_cache();
    let disabled = runtime.vm_register_lowering_cache_snapshot();
    assert!(!disabled.enabled);
    assert_eq!(disabled.cache_capacity, initial.cache_capacity);
    assert_eq!(disabled.cached_entries, 0);
    assert_eq!(disabled.hits, 0);
    assert_eq!(disabled.misses, 0);
    assert_eq!(disabled.build_errors, 0);
    assert_eq!(disabled.cache_evictions, 0);
    assert_eq!(disabled.invalidations, 0);

    runtime.set_vm_register_lowering_cache_enabled(true);
    assert!(runtime.vm_register_lowering_cache_snapshot().enabled);
}

#[test]
fn accessors_vm_tier1_toggle_and_reset_preserve_configuration() {
    let mut runtime = Runtime::new();
    let initial = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(initial.hot_block_threshold > 0);
    assert!(initial.cache_capacity > 0);

    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    let enabled = runtime.vm_tier1_specialized_executor_snapshot();
    assert!(enabled.enabled);
    assert_eq!(enabled.hot_block_threshold, initial.hot_block_threshold);
    assert_eq!(enabled.cache_capacity, initial.cache_capacity);
    assert_eq!(enabled.cached_blocks, 0);
    assert_eq!(enabled.compile_attempts, 0);
    assert_eq!(enabled.compile_successes, 0);
    assert_eq!(enabled.compile_failures, 0);
    assert_eq!(enabled.cache_evictions, 0);
    assert_eq!(enabled.block_executions, 0);
    assert_eq!(enabled.deopt_count, 0);

    runtime.set_vm_tier1_specialized_executor_enabled(false);
    assert!(!runtime.vm_tier1_specialized_executor_snapshot().enabled);
}

#[test]
fn accessors_vm_pou_name_requires_a_loaded_module() {
    let runtime = Runtime::new();

    assert_eq!(runtime.vm_pou_name(0), None);
    assert_eq!(runtime.vm_pou_name(u32::MAX), None);
}

#[test]
fn accessors_process_image_mutation_is_visible_through_immutable_view() {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(2, 3, 4);
    runtime.io_mut().inputs_mut().copy_from_slice(&[1, 2]);
    runtime.io_mut().outputs_mut().copy_from_slice(&[3, 4, 5]);
    runtime.io_mut().memory_mut().copy_from_slice(&[6, 7, 8, 9]);

    assert_eq!(runtime.io().inputs(), &[1, 2]);
    assert_eq!(runtime.io().outputs(), &[3, 4, 5]);
    assert_eq!(runtime.io().memory(), &[6, 7, 8, 9]);
}

#[test]
fn accessors_io_health_publication_replaces_snapshot_in_driver_order() {
    let mut runtime = Runtime::new();
    let sink = Arc::new(Mutex::new(vec![IoDriverStatus {
        name: "Stale".into(),
        health: IoDriverHealth::Faulted {
            error: "old".into(),
        },
    }]));
    runtime.set_io_health_sink(Some(sink.clone()));
    runtime.add_io_driver("Primary", Box::new(HealthDriver(IoDriverHealth::Ok)));
    runtime.add_io_driver(
        "Backup",
        Box::new(HealthDriver(IoDriverHealth::Degraded {
            error: "link".into(),
        })),
    );

    runtime.update_io_health();
    let snapshot = sink.lock().unwrap().clone();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].name, "Primary");
    assert_eq!(snapshot[0].health, IoDriverHealth::Ok);
    assert_eq!(snapshot[1].name, "Backup");
    assert_eq!(
        snapshot[1].health,
        IoDriverHealth::Degraded {
            error: "link".into(),
        }
    );

    runtime.clear_io_drivers();
    runtime.update_io_health();
    assert!(sink.lock().unwrap().is_empty());
}

#[test]
fn accessors_removing_io_health_sink_stops_publication() {
    let mut runtime = Runtime::new();
    let sink = Arc::new(Mutex::new(Vec::new()));
    runtime.set_io_health_sink(Some(sink.clone()));
    runtime.add_io_driver("Driver", Box::new(HealthDriver(IoDriverHealth::Ok)));
    runtime.update_io_health();
    assert_eq!(sink.lock().unwrap().len(), 1);

    runtime.set_io_health_sink(None);
    runtime.clear_io_drivers();
    runtime.update_io_health();
    assert_eq!(sink.lock().unwrap().len(), 1);
}

#[test]
fn accessors_empty_ads_lifecycle_and_hash_projection_are_stable() {
    let mut runtime = Runtime::new();
    assert_eq!(runtime.ads_connection_count(), 0);
    let initial = runtime.ads_status_report();
    assert_eq!(initial.overall, AdsStatusOverall::Disabled);
    assert!(initial.connections.is_empty());
    assert_eq!(initial.deployed_ads_config_hash, None);

    runtime.set_ads_deployed_config_hash(Some("sha256:ads".to_string()));
    assert_eq!(
        runtime
            .ads_status_report()
            .deployed_ads_config_hash
            .as_deref(),
        Some("sha256:ads")
    );
    runtime.shutdown_ads().unwrap();
    assert_eq!(runtime.ads_connection_count(), 0);
}

#[test]
fn accessors_empty_opcua_lifecycle_reset_clears_projection_and_hash() {
    let mut runtime = Runtime::new();
    assert_eq!(runtime.opcua_client_connection_count(), 0);
    let initial = runtime.opcua_client_status_report();
    assert!(!initial.enabled);
    assert!(initial.connections.is_empty());
    assert_eq!(initial.deployed_config_hash, None);

    runtime.set_opcua_client_deployed_config_hash(Some("sha256:opcua".to_string()));
    assert_eq!(
        runtime
            .opcua_client_status_report()
            .deployed_config_hash
            .as_deref(),
        Some("sha256:opcua")
    );

    runtime.reset_opcua_client_connections().unwrap();
    let reset = runtime.opcua_client_status_report();
    assert!(!reset.enabled);
    assert!(reset.connections.is_empty());
    assert_eq!(reset.deployed_config_hash, None);
}

#[test]
fn accessors_unconfigured_ads_has_no_active_device_snapshot() {
    let runtime = Runtime::new();
    let target = TargetIdentity {
        name: Some("PLC".to_string()),
        ip: "192.0.2.10".to_string(),
        ams_net_id: "1.2.3.4.5.6".to_string(),
        ams_port: 851,
        tc_version: None,
    };

    assert_eq!(runtime.active_ads_device_snapshot(&target, None), None);
}

#[test]
fn accessors_cycle_and_time_projection_return_exact_runtime_state() {
    let mut runtime = Runtime::new();
    runtime.current_time = Duration::from_nanos(-17);
    runtime.cycle_counter = u64::MAX;

    assert_eq!(runtime.current_time(), Duration::from_nanos(-17));
    assert_eq!(runtime.cycle_counter(), u64::MAX);
}

#[test]
fn accessors_var_access_map_preserves_exact_binding_identity() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("Data", Value::Word(0x1234));
    let reference = runtime.storage().ref_for_global("Data").unwrap();
    runtime.access_map_mut().bind(
        "DataByte".into(),
        reference.clone(),
        Some(PartialAccess::Byte(0)),
    );

    let binding = runtime.access_map().get("DataByte").unwrap();
    assert_eq!(binding.name, "DataByte");
    assert_eq!(binding.reference, reference);
    assert_eq!(binding.partial, Some(PartialAccess::Byte(0)));
    assert!(runtime.access_map().get("databyte").is_none());
}

#[test]
fn accessors_using_resolution_honors_fb_method_override_and_fallback() {
    let mut runtime = Runtime::new();
    runtime.register_function_block(function_block(
        "Valve",
        &["ValveLib"],
        &[method("Open", &["MethodLib"]), method("Close", &[])],
    ));
    let instance = runtime.storage_mut().create_instance("vAlVe");

    let override_frame = runtime
        .storage_mut()
        .push_frame_with_instance("oPeN", instance);
    assert_eq!(
        runtime.using_for_frame(override_frame),
        Some(vec!["MethodLib".into()])
    );
    runtime.storage_mut().pop_frame();

    let fallback_frame = runtime
        .storage_mut()
        .push_frame_with_instance("cLoSe", instance);
    assert_eq!(
        runtime.using_for_frame(fallback_frame),
        Some(vec!["ValveLib".into()])
    );
    runtime.storage_mut().pop_frame();

    let owner_frame = runtime
        .storage_mut()
        .push_frame_with_instance("VALVE", instance);
    assert_eq!(
        runtime.using_for_frame(owner_frame),
        Some(vec!["ValveLib".into()])
    );
}

#[test]
fn accessors_using_resolution_honors_class_method_override_and_fallback() {
    let mut runtime = Runtime::new();
    runtime.register_class(class(
        "Axis",
        &["AxisLib"],
        &[method("Move", &["MotionLib"]), method("Stop", &[])],
    ));
    let instance = runtime.storage_mut().create_instance("aXiS");

    let override_frame = runtime
        .storage_mut()
        .push_frame_with_instance("mOvE", instance);
    assert_eq!(
        runtime.using_for_frame(override_frame),
        Some(vec!["MotionLib".into()])
    );
    runtime.storage_mut().pop_frame();

    let fallback_frame = runtime
        .storage_mut()
        .push_frame_with_instance("sToP", instance);
    assert_eq!(
        runtime.using_for_frame(fallback_frame),
        Some(vec!["AxisLib".into()])
    );
}

#[test]
fn accessors_using_resolution_rejects_unknown_and_removed_frames() {
    let mut runtime = Runtime::new();
    let unknown_owner = runtime.storage_mut().push_frame("Unknown");
    assert_eq!(runtime.using_for_frame(unknown_owner), None);

    let removed = runtime.storage_mut().push_frame("Other");
    runtime.storage_mut().remove_frame(removed);
    assert_eq!(runtime.using_for_frame(removed), None);
    assert_eq!(runtime.using_for_frame(FrameId(u32::MAX)), None);
}

#[test]
fn accessors_var_access_reads_all_multibyte_projection_widths() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("Packed", Value::LWord(0x1122_3344_5566_7788));
    let reference = runtime.storage().ref_for_global("Packed").unwrap();
    for (name, partial) in [
        ("Byte", PartialAccess::Byte(1)),
        ("Word", PartialAccess::Word(1)),
        ("DWord", PartialAccess::DWord(1)),
    ] {
        runtime
            .access_map_mut()
            .bind(name.into(), reference.clone(), Some(partial));
    }

    assert_eq!(runtime.read_access("Byte"), Some(Value::Byte(0x77)));
    assert_eq!(runtime.read_access("Word"), Some(Value::Word(0x5566)));
    assert_eq!(
        runtime.read_access("DWord"),
        Some(Value::DWord(0x1122_3344))
    );
}

#[test]
fn accessors_var_access_writes_all_multibyte_projection_widths() {
    let cases = [
        (
            PartialAccess::Byte(1),
            Value::Byte(0xAA),
            Value::LWord(0x1122_3344_5566_AA88),
        ),
        (
            PartialAccess::Word(1),
            Value::Word(0xBBBB),
            Value::LWord(0x1122_3344_BBBB_7788),
        ),
        (
            PartialAccess::DWord(1),
            Value::DWord(0xCCCC_DDDD),
            Value::LWord(0xCCCC_DDDD_5566_7788),
        ),
    ];

    for (partial, replacement, expected) in cases {
        let mut runtime = Runtime::new();
        runtime
            .storage_mut()
            .set_global("Packed", Value::LWord(0x1122_3344_5566_7788));
        let reference = runtime.storage().ref_for_global("Packed").unwrap();
        runtime
            .access_map_mut()
            .bind("Part".into(), reference, Some(partial));

        runtime.write_access("Part", replacement).unwrap();
        assert_eq!(runtime.storage().get_global("Packed"), Some(&expected));
    }
}

#[test]
fn accessors_var_access_type_failures_are_atomic_for_every_projection_width() {
    for (partial, wrong_value) in [
        (PartialAccess::Bit(0), Value::Byte(1)),
        (PartialAccess::Byte(0), Value::Word(1)),
        (PartialAccess::Word(0), Value::DWord(1)),
        (PartialAccess::DWord(0), Value::LWord(1)),
    ] {
        let mut runtime = Runtime::new();
        let original = Value::LWord(0x1122_3344_5566_7788);
        runtime.storage_mut().set_global("Packed", original.clone());
        let reference = runtime.storage().ref_for_global("Packed").unwrap();
        runtime
            .access_map_mut()
            .bind("Part".into(), reference, Some(partial));

        assert_eq!(
            runtime.write_access("Part", wrong_value),
            Err(error::RuntimeError::TypeMismatch)
        );
        assert_eq!(runtime.storage().get_global("Packed"), Some(&original));
    }
}

#[test]
fn accessors_var_access_bounds_failures_are_atomic_for_every_projection_width() {
    for partial in [
        PartialAccess::Bit(64),
        PartialAccess::Byte(8),
        PartialAccess::Word(4),
        PartialAccess::DWord(2),
    ] {
        let mut runtime = Runtime::new();
        let original = Value::LWord(0x1122_3344_5566_7788);
        runtime.storage_mut().set_global("Packed", original.clone());
        let reference = runtime.storage().ref_for_global("Packed").unwrap();
        runtime
            .access_map_mut()
            .bind("Part".into(), reference, Some(partial));

        assert!(matches!(
            runtime.write_access("Part", projection_value(partial)),
            Err(error::RuntimeError::IndexOutOfBounds { .. })
        ));
        assert_eq!(runtime.read_access("Part"), None);
        assert_eq!(runtime.storage().get_global("Packed"), Some(&original));
    }
}

#[test]
fn accessors_var_access_partial_null_reference_is_atomic() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("Sentinel", Value::DInt(9));
    runtime.access_map_mut().bind(
        "Missing".into(),
        ValueRef {
            location: MemoryLocation::Global,
            offset: usize::MAX,
            path: RefPath::new(),
        },
        Some(PartialAccess::Bit(0)),
    );

    assert_eq!(runtime.read_access("Missing"), None);
    assert_eq!(
        runtime.write_access("Missing", Value::Bool(true)),
        Err(error::RuntimeError::NullReference)
    );
    assert_eq!(
        runtime.storage().get_global("Sentinel"),
        Some(&Value::DInt(9))
    );
}

#[derive(Debug)]
struct HealthDriver(IoDriverHealth);

impl IoDriver for HealthDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), error::RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), error::RuntimeError> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.0.clone()
    }
}

fn function(name: &str) -> FunctionDef {
    FunctionDef {
        name: name.into(),
        return_type: TypeId::DINT,
        params: Vec::new(),
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: Vec::new(),
    }
}

fn function_block(name: &str, using: &[&str], methods: &[MethodDef]) -> FunctionBlockDef {
    FunctionBlockDef {
        name: name.into(),
        base: None,
        interfaces: Vec::new(),
        params: Vec::new(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: using.iter().copied().map(Into::into).collect(),
        methods: methods.to_vec(),
        body: Vec::new(),
    }
}

fn class(name: &str, using: &[&str], methods: &[MethodDef]) -> ClassDef {
    ClassDef {
        name: name.into(),
        base: None,
        interfaces: Vec::new(),
        vars: Vec::new(),
        using: using.iter().copied().map(Into::into).collect(),
        methods: methods.to_vec(),
    }
}

fn interface(name: &str) -> InterfaceDef {
    InterfaceDef {
        name: name.into(),
        base: None,
        using: Vec::new(),
        methods: Vec::new(),
    }
}

fn method(name: &str, using: &[&str]) -> MethodDef {
    MethodDef {
        name: name.into(),
        return_type: None,
        params: Vec::new(),
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: using.iter().copied().map(Into::into).collect(),
        body: Vec::new(),
    }
}

fn program(name: &str, using: &[&str]) -> ProgramDef {
    ProgramDef {
        name: name.into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: using.iter().copied().map(Into::into).collect(),
        body: Vec::new(),
    }
}

fn projection_value(partial: PartialAccess) -> Value {
    match partial {
        PartialAccess::Bit(_) => Value::Bool(true),
        PartialAccess::Byte(_) => Value::Byte(1),
        PartialAccess::Word(_) => Value::Word(1),
        PartialAccess::DWord(_) => Value::DWord(1),
    }
}
