//! Spec 34 MEM-09: a scan's results and charges are identical whether its native calls read
//! their prepared plans or resolve every argument and callee at the call. The `blocks` and
//! `methods` fixtures exercise function-block, built-in and method dispatch; `calls` exercises
//! user functions and standard functions.
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule, RuntimeState};

const CALLS: &[u8] = include_bytes!("fixtures/bounded-c/calls/program.stbc");
const BLOCKS: &[u8] = include_bytes!("fixtures/bounded-c/blocks/program.stbc");
const METHODS: &[u8] = include_bytes!("fixtures/bounded-c/methods/program.stbc");

/// The chips' execution services: no UTC clock and a deadline that never expires.
struct Services;

impl ExecutionServices for Services {
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported("no UTC clock".into()))
    }
}

/// Run `scans` cycles of one fixture under `max_work`, with or without prepared call plans.
fn run(bytes: &[u8], scans: i64, max_work: usize, plans: bool) -> Result<(), RuntimeError> {
    PreparedModule::set_call_plans_enabled(plans);
    let prepared = PreparedModule::from_bytes(
        bytes,
        PreparationLimits {
            max_work,
            ..Default::default()
        },
    )?;
    let services = Services;
    let mut state = prepared.instantiate_with_services(0, &services)?;
    for scan in 1..=scans {
        state.execute_cycle(Duration::from_millis(10 * scan))?;
    }
    Ok(())
}

/// Run `scans` cycles and return the program's final variables.
fn run_summary(
    bytes: &[u8],
    scans: i64,
    max_work: usize,
    plans: bool,
    program: &str,
) -> Result<Vec<(String, Value)>, RuntimeError> {
    PreparedModule::set_call_plans_enabled(plans);
    let prepared = PreparedModule::from_bytes(
        bytes,
        PreparationLimits {
            max_work,
            ..Default::default()
        },
    )?;
    let services = Services;
    let mut state = prepared.instantiate_with_services(0, &services)?;
    for scan in 1..=scans {
        state.execute_cycle(Duration::from_millis(10 * scan))?;
    }
    Ok(summary(&state, program))
}

/// Every variable of the program instance, plus the `ticks` global, in a stable order.
fn summary(state: &RuntimeState<'_>, program: &str) -> Vec<(String, Value)> {
    let Some(Value::Instance(root)) = state.storage().get_global(program) else {
        panic!("program {program} is not an instance");
    };
    let instance = state
        .storage()
        .get_instance(*root)
        .expect("the program instance is live");
    let mut values: Vec<(String, Value)> = instance
        .variables
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect();
    if let Some(ticks) = state.storage().get_global("ticks") {
        values.push(("ticks".to_owned(), ticks.clone()));
    }
    values.sort_by(|left, right| left.0.cmp(&right.0));
    values
}

/// The smallest scan allowance that admits `scans` consecutive cycles.
fn minimal_allowance(bytes: &[u8], scans: i64, plans: bool) -> usize {
    let (mut low, mut high) = (1usize, 10_000_000usize);
    while low < high {
        let middle = low + (high - low) / 2;
        match run(bytes, scans, middle, plans) {
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

/// 101 scans of `calls`, `blocks` and `methods` hold the same results and the same charge
/// boundary with prepared plans and with every call resolved at the call site.
#[test]
fn call_plans_change_no_result_or_charge_of_any_scan() {
    let fixtures = [
        ("calls", CALLS, "Plant"),
        ("blocks", BLOCKS, "Plant"),
        ("methods", METHODS, "Plant"),
    ];
    for (name, bytes, program) in fixtures {
        let planned = run_summary(bytes, 101, 10_000_000, true, program)
            .expect("the planned scans are admitted");
        let resolved = run_summary(bytes, 101, 10_000_000, false, program)
            .expect("the resolved scans are admitted");
        assert_eq!(
            planned, resolved,
            "{name}: the two arms hold different results after 101 scans"
        );
        assert_eq!(
            minimal_allowance(bytes, 101, true),
            minimal_allowance(bytes, 101, false),
            "{name}: the two arms refuse at different scan allowances"
        );
    }
}
