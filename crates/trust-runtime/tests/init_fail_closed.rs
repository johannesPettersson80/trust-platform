use trust_runtime::debug::RuntimeEvent;
use trust_runtime::eval::expr::LValue;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

#[test]
fn interface_param_defaults_to_null_reference() {
    let source = r#"
INTERFACE I_Svc
END_INTERFACE

FUNCTION_BLOCK Consumer
VAR_INPUT
    Svc : I_Svc;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    C : Consumer;
END_VAR
END_PROGRAM
"#;

    let harness = TestHarness::from_source(source).expect("interface default should compile");
    let storage = harness.runtime().storage();
    let main_id = match storage.get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main program instance, got {other:?}"),
    };
    let consumer_id = match storage.get_instance_var(main_id, "C") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main.C function-block instance, got {other:?}"),
    };

    assert_eq!(
        storage.get_instance_var(consumer_id, "Svc"),
        Some(&Value::Null),
        "an interface variable without an explicit initializer starts as NULL"
    );
}

#[test]
fn debug_queued_lvalue_write_failure_is_observable() {
    let mut runtime = Runtime::new();
    let debug = runtime.enable_debug();
    debug.enqueue_lvalue_write(None, LValue::Name("missing".into()), Value::DInt(7));

    let err = runtime
        .execute_cycle()
        .expect_err("queued debug write to missing target must fail");
    assert!(
        err.to_string().contains("missing"),
        "expected missing target context, got {err}"
    );
    assert!(runtime.storage().get_global("missing").is_none());
    let events = debug.drain_runtime_events();
    assert!(
        events.iter().any(
            |event| matches!(event, RuntimeEvent::Fault { error, .. } if error.contains("missing"))
        ),
        "expected runtime fault event for debug write failure, got {events:?}"
    );
}

#[test]
fn debug_queued_global_write_unknown_target_fails() {
    let mut runtime = Runtime::new();
    let debug = runtime.enable_debug();
    debug.enqueue_global_write("missing", Value::DInt(7));

    let err = runtime
        .execute_cycle()
        .expect_err("queued debug global write to missing target must fail");
    assert!(
        err.to_string().contains("missing"),
        "expected missing target context, got {err}"
    );
    assert!(runtime.storage().get_global("missing").is_none());
}
