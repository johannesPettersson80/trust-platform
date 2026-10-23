//! TIME-09: a nested method call and its call-time REF initializer run inside the scan allowance.
//! The portability fixture's `Counter.Advance` declares `previous : REF_TO INT := REF(history[delta])`,
//! so that initializer is re-run on every call with the changing input.
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::memory::VariableStorage;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const PROGRAM_V2: &[u8] = include_bytes!("fixtures/portability/stbc-2.0/program-v2.stbc");

fn plant_field(storage: &VariableStorage, name: &str) -> Value {
    let Some(Value::Instance(plant)) = storage.get_global("Plant") else {
        panic!("program instance Plant")
    };
    storage
        .get_instance_var(*plant, name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .clone()
}

/// Runs `scans` cycles of the fixture at its 25 ms interval under `max_work`. Returns the
/// `(activations, result)` the program holds after the last cycle, or the first refusal.
fn run_scans(max_work: usize, scans: i64) -> Result<(Value, Value), RuntimeError> {
    let prepared = PreparedModule::from_bytes(
        PROGRAM_V2,
        PreparationLimits {
            max_work,
            ..Default::default()
        },
    )
    .expect("admit the portability fixture");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the fixture");
    for scan in 1..=scans {
        state.execute_cycle(Duration::from_millis(25 * scan))?;
    }
    let storage = state.storage();
    Ok((
        plant_field(storage, "activations"),
        plant_field(storage, "result"),
    ))
}

/// The smallest scan allowance that admits `scans` consecutive cycles from a fresh state.
fn minimal_allowance(scans: i64) -> usize {
    let (mut low, mut high) = (1usize, 1_000_000usize);
    while low < high {
        let middle = low + (high - low) / 2;
        match run_scans(middle, scans) {
            Ok(_) => high = middle,
            Err(error) => {
                assert!(
                    matches!(
                        error,
                        RuntimeError::ExecutionLimit {
                            kind: ExecutionLimitKind::Work,
                            ..
                        }
                    ),
                    "a refused scan must be a work refusal, got {error:?}"
                );
                low = middle + 1;
            }
        }
    }
    low
}

#[test]
fn nested_method_call_and_its_call_time_initializer_run_inside_the_scan_allowance() {
    let one_scan = minimal_allowance(1);

    // Admitted at its boundary: the nested call ran, and REF(history[1]) selected 1.
    let (activations, result) = run_scans(one_scan, 1).expect("the measured scan is admitted");
    assert_eq!(activations, Value::DInt(1));
    assert_eq!(result, Value::Int(1));

    // One unit less is refused as a work refusal of the scan's own allowance.
    let error = run_scans(one_scan - 1, 1).expect_err("one unit below the boundary is refused");
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            limit,
            ..
        } if limit == one_scan - 1
    ));

    // The second cycle re-runs the initializer with the changed input: REF(history[2]) selects 2.
    let two_scans = minimal_allowance(2);
    let (activations, result) =
        run_scans(two_scans, 2).expect("each scan starts a fresh allowance");
    assert_eq!(activations, Value::DInt(2));
    assert_eq!(result, Value::Int(2));
}
