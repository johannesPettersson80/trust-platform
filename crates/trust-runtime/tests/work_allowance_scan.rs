//! TIME-08 / TIME-09 scan allowance: each scan starts a fresh allowance, scheduled invocations
//! keep the single-invocation boundary, and later scans never cost more than the first.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::memory::VariableStorage;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

fn scheduled(body: &str) -> String {
    format!(
        "{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, \
         PRIORITY := 0); PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION"
    )
}

const COUNTER_MAIN: &str =
    "PROGRAM Main VAR count : INT; END_VAR count := count + INT#1; END_PROGRAM";

/// Every scan runs the same instructions. The program has array element and index lookups and a
/// nested call to a function block whose field is read and written by name. It has no
/// data-dependent branch: `i` cycles through 0..3 with MOD, so the element changes, not the path.
const TABLE_MAIN: &str = "VAR_GLOBAL table : ARRAY[0..3] OF INT; END_VAR \
    FUNCTION_BLOCK Tally VAR count : INT; END_VAR count := count + INT#1; END_FUNCTION_BLOCK \
    PROGRAM Main VAR counter_fb : Tally; i : INT; total : INT; END_VAR \
    table[i] := table[i] + INT#1; \
    total := total + table[INT#1]; \
    i := (i + INT#1) MOD INT#4; \
    counter_fb(); \
    END_PROGRAM";

/// Runs the scans at `times_ms` under `max_work` and reports whether every one was admitted.
/// Only a work refusal may fail a scan.
fn scans_admitted(bytes: &[u8], max_work: usize, times_ms: &[i64]) -> bool {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(bytes, limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    for &millis in times_ms {
        if let Err(error) = state.execute_cycle(Duration::from_millis(millis)) {
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
            return false;
        }
    }
    true
}

/// The smallest scan allowance that admits every scan in `times_ms` from a fresh state.
/// Admission is monotone in the allowance, so a binary search finds the boundary.
fn minimal_allowance(bytes: &[u8], times_ms: &[i64]) -> usize {
    let (mut low, mut high) = (1usize, 1_000_000usize);
    assert!(
        scans_admitted(bytes, high, times_ms),
        "the upper probe must admit the scans"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if scans_admitted(bytes, middle, times_ms) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn program_field(storage: &VariableStorage, program: &str, name: &str) -> Value {
    let Some(Value::Instance(id)) = storage.get_global(program) else {
        panic!("program instance {program}")
    };
    storage
        .get_instance_var(*id, name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .clone()
}

#[test]
fn each_scan_starts_a_fresh_allowance_so_two_scans_fit_just_above_one_scan_of_work() {
    let bytes = artifact(&scheduled(COUNTER_MAIN));
    let one_scan = minimal_allowance(&bytes, &[10]);
    // The second scan cannot draw on the first scan's units: one scan plus one unit admits both.
    assert!(scans_admitted(&bytes, one_scan + 1, &[10, 20]));
}

#[test]
fn repeated_scans_never_cost_more_than_the_first_as_index_and_name_caches_warm() {
    let bytes = artifact(&scheduled(TABLE_MAIN));
    let first_scan = minimal_allowance(&bytes, &[10]);
    // Each scan takes the same path, so the boundary for the whole sequence must equal the first
    // scan's boundary: warm index, name and lowering caches must not change the work or the
    // refusal point.
    assert_eq!(
        minimal_allowance(&bytes, &[10, 20, 30, 40]),
        first_scan,
        "a later scan charged more work than the first"
    );
    assert!(!scans_admitted(&bytes, first_scan - 1, &[10]));
}

#[test]
fn two_ready_tasks_keep_the_single_invocation_boundary() {
    let single = artifact(&scheduled(COUNTER_MAIN));
    let one_invocation = minimal_allowance(&single, &[10]);
    // One PROGRAM type cannot be instantiated twice, so each task runs its own type with the
    // same body, which costs the same as `Main`.
    let two_tasks = artifact(
        "PROGRAM Fast_Main VAR count : INT; END_VAR count := count + INT#1; END_PROGRAM \
         PROGRAM Slow_Main VAR count : INT; END_VAR count := count + INT#1; END_PROGRAM \
         CONFIGURATION Conf RESOURCE Controller ON PLC \
         TASK Fast (INTERVAL := T#10ms, PRIORITY := 0); \
         TASK Slow (INTERVAL := T#10ms, PRIORITY := 1); \
         PROGRAM Plant WITH Fast : Fast_Main; PROGRAM Other WITH Slow : Slow_Main; \
         END_RESOURCE END_CONFIGURATION",
    );

    // Both invocations are due in the same cycle, and ordering them draws nothing: each keeps
    // the boundary of one invocation.
    assert!(scans_admitted(&two_tasks, one_invocation, &[10]));
    assert!(!scans_admitted(&two_tasks, one_invocation - 1, &[10]));

    let prepared = PreparedModule::from_bytes(
        &two_tasks,
        PreparationLimits {
            max_work: one_invocation,
            ..Default::default()
        },
    )
    .expect("admit the two-task artifact");
    let mut state = prepared.instantiate(0).expect("instantiate the two tasks");
    state
        .execute_cycle(Duration::from_millis(10))
        .expect("both invocations fit the boundary");
    assert_eq!(
        program_field(state.storage(), "Plant", "count"),
        Value::Int(1)
    );
    assert_eq!(
        program_field(state.storage(), "Other", "count"),
        Value::Int(1)
    );
}
