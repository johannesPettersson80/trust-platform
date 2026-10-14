//! Retained values and PLC instances across a resource-only update (spec 34 HOST-09).
use trust_runtime::{bytecode::ResourceMetadata, harness::CompileSession, value::Value};

const RETAINED: &str = "VAR_GLOBAL RETAIN count : DINT := DINT#3; END_VAR VAR_GLOBAL transient : DINT := DINT#4; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM";

const INSTANCE: &str = "PROGRAM Main VAR value : INT := INT#3; END_VAR value := value + INT#1; END_PROGRAM CONFIGURATION C PROGRAM P : Main; END_CONFIGURATION";

/// Spec 34 HOST-09, resource-only row: retained values stay exactly as they were across the
/// update, and the next cycle continues from them rather than from the declared default.
#[test]
fn resource_only_update_keeps_retained_global_values() {
    let mut installed = CompileSession::from_source(RETAINED)
        .build_runtime()
        .unwrap();
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed
            .storage()
            .get_retain("count")
            .or(installed.storage().get_global("count")),
        Some(&Value::DInt(4))
    );
    let retained = installed.storage().retain().clone();
    let globals = installed.storage().globals().clone();
    let mut resource = ResourceMetadata {
        name: "R".into(),
        process_image: Default::default(),
        tasks: Vec::new(),
    };
    resource.process_image.outputs = 8;
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(installed.storage().retain(), &retained);
    assert_eq!(installed.storage().globals(), &globals);
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed
            .storage()
            .get_retain("count")
            .or(installed.storage().get_global("count")),
        Some(&Value::DInt(5))
    );
}

/// Spec 34 HOST-09, resource-only row: the update does not rerun initializers and does not
/// replace the PLC instance graph. The instance keeps its identity and its run-time value,
/// so the next cycle continues from 4 (an initializer rerun would restore INT#3 and give 4).
#[test]
fn resource_only_update_does_not_rerun_initializers_or_replace_the_instance_graph() {
    let mut installed = CompileSession::from_source(INSTANCE)
        .build_runtime()
        .unwrap();
    installed.execute_cycle().unwrap();
    let Some(Value::Instance(program)) = installed.storage().get_global("P").cloned() else {
        panic!("program instance")
    };
    assert_eq!(
        installed.storage().get_instance_var(program, "value"),
        Some(&Value::Int(4))
    );
    let mut resource = ResourceMetadata {
        name: "R".into(),
        process_image: Default::default(),
        tasks: Vec::new(),
    };
    resource.process_image.outputs = 8;
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(
        installed.storage().get_global("P"),
        Some(&Value::Instance(program))
    );
    assert_eq!(
        installed.storage().get_instance_var(program, "value"),
        Some(&Value::Int(4))
    );
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_instance_var(program, "value"),
        Some(&Value::Int(5))
    );
}
