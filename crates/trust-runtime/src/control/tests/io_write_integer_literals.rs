// Issue #135 and spec 11 ("An admitted integer is an untyped literal"): a queued
// io.write or io.force takes its destination's declared type at the scan boundary.
// A value outside that type is rejected alone, as write_rejected, and the PLC keeps
// running.

const IO_LITERAL_SOURCE: &str = r#"
PROGRAM MainProgram
VAR
    Counter : INT;
    Spare : BYTE;
END_VAR
END_PROGRAM

CONFIGURATION IoLiteralConfig
PROGRAM P1 : MainProgram;
VAR_CONFIG
    P1.Counter AT %MW0 : INT;
    P1.Spare AT %MB15 : BYTE;
END_VAR
END_CONFIGURATION
"#;

fn io_literal_fixture() -> (ControlState, crate::Runtime) {
    let state = hmi_test_state(IO_LITERAL_SOURCE);
    let mut runtime = crate::harness::CompileSession::from_source(IO_LITERAL_SOURCE)
        .build_runtime()
        .expect("io literal fixture compiles");
    runtime.set_debug_control(state.debug.clone());
    (state, runtime)
}

fn io_literal_request(
    state: &ControlState,
    id: u64,
    kind: &str,
    address: &str,
    value: &str,
) -> ControlResponse {
    let response = handle_request_value(
        json!({"id": id, "type": kind, "params": {"address": address, "value": value}}),
        state,
        None,
    );
    assert!(
        response.ok,
        "{kind} {address} {value} should queue: {:?}",
        response.error
    );
    response
}

fn io_literal_image(runtime: &crate::Runtime, address: &str) -> crate::value::Value {
    let address = crate::io::IoAddress::parse(address).expect("address parses");
    runtime.io().read(&address).expect("process image reads")
}

fn io_literal_counter(runtime: &crate::Runtime) -> crate::value::Value {
    let Some(crate::value::Value::Instance(program)) = runtime.storage().get_global("P1") else {
        panic!("program instance P1");
    };
    runtime
        .storage()
        .get_instance_var(*program, "Counter")
        .cloned()
        .expect("Counter")
}

/// Drain the write_rejected events raised since the previous call.
fn io_literal_rejections(state: &ControlState) -> usize {
    state
        .debug
        .drain_runtime_events()
        .iter()
        .filter(|event| matches!(event, crate::debug::RuntimeEvent::WriteRejected { .. }))
        .count()
}

#[test]
fn io_write_integer_literal_takes_the_declared_int_type() {
    use crate::value::Value;
    let (state, mut runtime) = io_literal_fixture();
    io_literal_request(&state, 1, "io.write", "%MW0", "41");

    runtime
        .execute_cycle()
        .expect("the INT literal is accepted at the scan boundary");

    assert_eq!(io_literal_image(&runtime, "%MW0"), Value::Word(41));
    assert_eq!(io_literal_counter(&runtime), Value::Int(41));
    assert!(
        !runtime.faulted(),
        "an accepted literal must not fault the PLC"
    );
    assert_eq!(io_literal_rejections(&state), 0);
}

#[test]
fn io_write_integer_literal_outside_the_declared_type_is_rejected_without_a_fault() {
    use crate::value::Value;
    let (state, mut runtime) = io_literal_fixture();
    io_literal_request(&state, 2, "io.write", "%MW0", "40000");

    runtime
        .execute_cycle()
        .expect("a rejected write must not fault the scan");

    assert_eq!(io_literal_rejections(&state), 1, "one write_rejected event");
    assert_eq!(
        io_literal_image(&runtime, "%MW0"),
        Value::Word(0),
        "the rejected write leaves the destination unchanged"
    );
    assert_eq!(io_literal_counter(&runtime), Value::Int(0));
    assert!(!runtime.faulted());

    runtime
        .execute_cycle()
        .expect("the PLC keeps scanning after the rejection");
    assert_eq!(runtime.cycle_counter(), 2);
}

#[test]
fn io_force_integer_literal_forces_the_declared_int_binding() {
    use crate::value::Value;
    let (state, mut runtime) = io_literal_fixture();
    io_literal_request(&state, 3, "io.force", "%MW0", "41");

    runtime
        .execute_cycle()
        .expect("the forced INT literal is applied at the scan boundary");

    assert_eq!(io_literal_image(&runtime, "%MW0"), Value::Word(41));
    assert_eq!(io_literal_counter(&runtime), Value::Int(41));
    let forced = crate::io::IoAddress::parse("%MW0").expect("address parses");
    assert!(
        state
            .debug
            .forced_snapshot()
            .io
            .iter()
            .any(|(address, _)| *address == forced),
        "the force stays active for the next scan"
    );
    assert_eq!(io_literal_rejections(&state), 0);
    assert!(!runtime.faulted());
}

#[test]
fn io_write_unbound_byte_accepts_200_and_rejects_300_and_minus_one() {
    use crate::value::Value;
    let (state, mut runtime) = io_literal_fixture();
    io_literal_request(&state, 4, "io.write", "%MB10", "200");

    runtime
        .execute_cycle()
        .expect("an unbound BYTE accepts a literal in range");

    assert_eq!(io_literal_image(&runtime, "%MB10"), Value::Byte(200));
    assert_eq!(io_literal_rejections(&state), 0);

    for (id, literal) in [(5, "300"), (6, "-1")] {
        io_literal_request(&state, id, "io.write", "%MB10", literal);
        runtime
            .execute_cycle()
            .expect("a rejected byte must not fault the scan");
        assert_eq!(
            io_literal_image(&runtime, "%MB10"),
            Value::Byte(200),
            "{literal} must leave the byte unchanged"
        );
        assert_eq!(
            io_literal_rejections(&state),
            1,
            "{literal} reports one write_rejected event"
        );
    }
    assert!(!runtime.faulted());
}

#[test]
fn io_write_bit_address_accepts_truth_values_and_rejects_integer_literals() {
    use crate::value::Value;
    let (state, mut runtime) = io_literal_fixture();
    io_literal_request(&state, 7, "io.write", "%MX14.0", "TRUE");

    runtime.execute_cycle().expect("a bit address accepts TRUE");

    assert_eq!(io_literal_image(&runtime, "%MX14.0"), Value::Bool(true));
    assert_eq!(io_literal_rejections(&state), 0);

    io_literal_request(&state, 8, "io.write", "%MX14.0", "1");
    runtime
        .execute_cycle()
        .expect("a rejected bit write must not fault the scan");

    assert_eq!(
        io_literal_image(&runtime, "%MX14.0"),
        Value::Bool(true),
        "a bit address takes no integer literal, so the bit is unchanged"
    );
    assert_eq!(io_literal_rejections(&state), 1);
    assert!(!runtime.faulted());
}
