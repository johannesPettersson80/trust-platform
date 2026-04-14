use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn fb_instance_runs_under_task_control() {
    let source = r#"
FUNCTION_BLOCK FB
VAR_INPUT
    IN : BOOL;
END_VAR
VAR_OUTPUT
    OUT : BOOL;
END_VAR
OUT := IN;
END_FUNCTION_BLOCK

PROGRAM P
VAR
    fb : FB;
END_VAR
END_PROGRAM

CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    trigger : BOOL;
END_VAR
TASK T (SINGLE := trigger, INTERVAL := T#0ms, PRIORITY := 0);
PROGRAM P1 WITH T : P (fb WITH T);
END_RESOURCE
END_CONFIGURATION
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let program_id = match harness.runtime().storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected program instance, got {other:?}"),
    };
    let fb_id = match harness
        .runtime()
        .storage()
        .get_instance_var(program_id, "fb")
    {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected FB instance, got {other:?}"),
    };

    {
        let runtime = harness.runtime_mut();
        runtime
            .storage_mut()
            .set_instance_var(fb_id, "IN", Value::Bool(true));
        runtime
            .storage_mut()
            .set_global("trigger", Value::Bool(true));
    }

    harness.cycle();

    let out = harness.runtime().storage().get_instance_var(fb_id, "OUT");
    assert_eq!(out, Some(&Value::Bool(true)));
}
