//! Pending debugger requests across a resource-only update (spec 34 HOST-09).
use trust_runtime::{
    bytecode::ResourceMetadata,
    harness::CompileSession,
    task::TaskConfig,
    value::{Duration, Value},
};

const SOURCE: &str = "PROGRAM Main VAR value : INT := INT#3; END_VAR value := value + INT#1; END_PROGRAM CONFIGURATION C VAR_GLOBAL trigger : BOOL := FALSE; END_VAR PROGRAM P : Main; END_CONFIGURATION";

/// Spec 34 HOST-09, resource-only row: the resource-only update keeps the debugger's pending
/// requests. A write queued before the update is not applied by the update itself, and the
/// next cycle applies it.
#[test]
fn resource_only_update_keeps_a_queued_debugger_write_for_the_next_cycle() {
    let mut installed = CompileSession::from_source(SOURCE).build_runtime().unwrap();
    let debug = installed.enable_debug();
    debug.enqueue_global_write("trigger", Value::Bool(true));
    let resource = ResourceMetadata {
        name: "R".into(),
        process_image: Default::default(),
        tasks: vec![TaskConfig {
            name: "T".into(),
            interval: Duration::ZERO,
            single: Some("trigger".into()),
            priority: 0,
            programs: Vec::new(),
            fb_instances: Vec::new(),
        }],
    };
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(
        installed.storage().get_global("trigger"),
        Some(&Value::Bool(false))
    );
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_global("trigger"),
        Some(&Value::Bool(true))
    );
}
