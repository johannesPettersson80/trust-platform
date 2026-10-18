//! Scalar reads on the runtime state, the reads the board fixture now makes instead of
//! borrowing the whole store (slice C1-9 P1). The expected values are worked out by hand from the
//! fixture programs; they are not taken from the implementation.
use trust_runtime_core::{
    error::RuntimeError,
    value::{Duration, Value},
    vm::{PreparationLimits, PreparedModule},
};

/// `f401/gpio.st`: `probe_depth : INT := INT#1` (a resource global), and program `Plant` on a
/// 10 ms task with `depth_result : INT`.
const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

/// `stbc-2.0/main.st`: program `Plant` on a 25 ms task with `counter : Counter` (its `value` is
/// `INT#3`, and its `history` is an `ARRAY[1..3] OF INT`), `delay : TON` and `result : INT`.
const MAIN: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

#[test]
fn scalar_read_of_an_undeclared_name_is_none() {
    // Wrong behaviour caught: a default value (for example `Int(0)`) or an error returned in
    // place of `None` for a name the program does not declare.
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let state = prepared.instantiate(0).unwrap();
    let plant = state
        .read_global_instance("Plant")
        .unwrap()
        .expect("program Plant is a resource global");
    assert_eq!(state.read_global_scalar("no_such_global"), Ok(None));
    assert_eq!(
        state.read_instance_scalar(plant, "no_such_member"),
        Ok(None)
    );
    assert_eq!(state.read_global_instance("no_such_global"), Ok(None));
    // Control: a declared global is read, with its declared initial value `INT#1`.
    assert_eq!(
        state.read_global_scalar("probe_depth"),
        Ok(Some(Value::Int(1)))
    );
}

#[test]
fn scalar_read_of_a_structure_is_refused() {
    // Wrong behaviour caught: an instance, an array or a structure returned as a scalar, or read
    // as a default. Only the refusal `TypeMismatch` is correct for each of them.
    let prepared = PreparedModule::from_bytes(MAIN, PreparationLimits::default()).unwrap();
    let state = prepared.instantiate(0).unwrap();
    let plant = state
        .read_global_instance("Plant")
        .unwrap()
        .expect("program Plant is a resource global");
    let counter = state
        .read_instance_member_instance(plant, "counter")
        .unwrap()
        .expect("program Plant declares counter");
    // The program instance `Plant` and the function-block instance `counter` are instances; the
    // ARRAY `history` is a member of `counter`. None of them is an elementary scalar.
    assert_eq!(
        state.read_global_scalar("Plant"),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        state.read_instance_scalar(plant, "counter"),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        state.read_instance_scalar(counter, "history"),
        Err(RuntimeError::TypeMismatch)
    );
    // Control: a scalar member of the same instance is still read, with `INT#3`.
    assert_eq!(
        state.read_instance_scalar(counter, "value"),
        Ok(Some(Value::Int(3)))
    );
}

#[test]
fn scalar_read_returns_the_value_the_last_scan_stored() {
    // gpio.st: `CASE probe_depth`: 2 gives `ProbeLeaf() + 1` = 1 + 1 = 2; 3 gives
    // `ProbeMiddle() + 1` = (1 + 1) + 1 = 3; any other value gives 1. The 10 ms task first runs at
    // 10 ms, one interval after its baseline at 0 ms, so the scan at 0 ms stores nothing.
    // Wrong behaviour caught: the initial value after a scan has run, a stale value after a later
    // scan, or an engineering write read before the next scan stores it.
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let plant = state
        .read_global_instance("Plant")
        .unwrap()
        .expect("program Plant is a resource global");
    // No scan has stored anything: `depth_result : INT` holds its declared default, 0.
    assert_eq!(
        state.read_instance_scalar(plant, "depth_result"),
        Ok(Some(Value::Int(0)))
    );
    // The scan at 10 ms runs with probe_depth = 1, so the ELSE branch stores 1.
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(
        state.read_instance_scalar(plant, "depth_result"),
        Ok(Some(Value::Int(1)))
    );
    // The engineering write changes the global at once, but no scan has run since, so
    // depth_result still holds the value the scan at 10 ms stored.
    state.write_global("probe_depth", Value::Int(2)).unwrap();
    assert_eq!(
        state.read_global_scalar("probe_depth"),
        Ok(Some(Value::Int(2)))
    );
    assert_eq!(
        state.read_instance_scalar(plant, "depth_result"),
        Ok(Some(Value::Int(1)))
    );
    // The scan at 20 ms runs with probe_depth = 2: ProbeLeaf() + 1 = 2.
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(
        state.read_instance_scalar(plant, "depth_result"),
        Ok(Some(Value::Int(2)))
    );
    // The scan at 30 ms runs with probe_depth = 3: ProbeMiddle() + 1 = 3.
    state.write_global("probe_depth", Value::Int(3)).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    assert_eq!(
        state.read_instance_scalar(plant, "depth_result"),
        Ok(Some(Value::Int(3)))
    );
}
