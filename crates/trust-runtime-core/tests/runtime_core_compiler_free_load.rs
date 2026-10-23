//! Saved STBC only: this target neither authors source nor links HIR.
use trust_runtime_core::memory::{InstanceId, VariableStorage};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const ARTIFACT: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const TRACE: &str =
    include_str!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/expected-a4-trace.csv");
fn instance(value: Option<&Value>) -> InstanceId {
    match value {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected instance, got {other:?}"),
    }
}
fn field<'a>(storage: &'a VariableStorage, owner: InstanceId, name: &str) -> &'a Value {
    storage
        .get_instance_var(owner, name)
        .unwrap_or_else(|| panic!("missing {name}"))
}
fn snapshot(storage: &VariableStorage) -> (Value, Value, Value, Value, Value) {
    let plant = instance(storage.get_global("Plant"));
    let counter = instance(Some(field(storage, plant, "counter")));
    let timer = instance(Some(field(storage, plant, "delay")));
    (
        field(storage, counter, "value").clone(),
        field(storage, plant, "result").clone(),
        field(storage, plant, "activations").clone(),
        field(storage, timer, "Q").clone(),
        field(storage, timer, "ET").clone(),
    )
}
#[test]
fn saved_artifact_executes_expected_trace_without_compiler_or_source() {
    let prepared = PreparedModule::from_bytes(ARTIFACT, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for row in TRACE.lines().filter(|line| !line.starts_with('#')).skip(1) {
        let n: Vec<i64> = row.split(',').map(|cell| cell.parse().unwrap()).collect();
        state.execute_cycle(Duration::from_millis(n[0])).unwrap();
        assert_eq!(
            snapshot(state.storage()),
            (
                Value::Int(n[1] as i16),
                Value::Int(n[2] as i16),
                Value::DInt(n[3] as i32),
                Value::Bool(n[4] != 0),
                Value::Time(Duration::from_millis(n[5]))
            ),
            "sample {}",
            n[0]
        );
        assert_eq!(state.task_states()[0].overrun_count, 0);
        assert_eq!(state.task_states()[0].last_run, Duration::from_millis(n[6]));
        assert!(state.fault().is_none());
    }
    state.restart(RestartMode::Warm).unwrap();
    assert_eq!(snapshot(state.storage()).0, Value::Int(3));
    assert_eq!(snapshot(state.storage()).2, Value::DInt(40));
    state.execute_cycle(Duration::from_millis(1030)).unwrap();
    assert_eq!(
        (
            snapshot(state.storage()).0,
            snapshot(state.storage()).1,
            snapshot(state.storage()).2
        ),
        (Value::Int(5), Value::Int(2), Value::DInt(41))
    );
    state.restart(RestartMode::Cold).unwrap();
    state.execute_cycle(Duration::from_millis(1060)).unwrap();
    assert_eq!(
        (
            snapshot(state.storage()).0,
            snapshot(state.storage()).1,
            snapshot(state.storage()).2
        ),
        (Value::Int(4), Value::Int(1), Value::DInt(1))
    );
}
