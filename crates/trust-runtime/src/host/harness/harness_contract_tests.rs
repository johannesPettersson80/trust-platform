use super::*;

use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn counter_source() -> &'static str {
    r#"
PROGRAM Main
VAR
    count : DINT;
END_VAR
count := count + 1;
END_PROGRAM
"#
}

fn helper_source() -> &'static str {
    r#"
FUNCTION AddOne : DINT
VAR_INPUT
    value : DINT;
END_VAR
AddOne := value + 1;
END_FUNCTION
"#
}

fn counter_with_helper_source() -> &'static str {
    r#"
PROGRAM Main
VAR
    count : DINT;
END_VAR
count := AddOne(count);
END_PROGRAM
"#
}

fn io_source() -> &'static str {
    r#"
PROGRAM Main
VAR
    input : BOOL;
    output : BOOL;
END_VAR
output := input;
END_PROGRAM
"#
}

fn runtime_error_source() -> &'static str {
    r#"
PROGRAM Main
VAR
    result : DINT;
END_VAR
result := 1 / 0;
END_PROGRAM
"#
}

fn retained_source(initial: i16) -> String {
    format!(
        r#"
CONFIGURATION Conf
VAR_GLOBAL RETAIN
    counter : INT := INT#{initial};
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION

PROGRAM Main
END_PROGRAM
"#
    )
}

#[test]
fn harness_lifecycle_contract_from_source_starts_at_zero_without_cycle() {
    let harness = TestHarness::from_source(counter_source()).unwrap();
    assert_eq!(harness.cycle_count(), 0);
    assert_eq!(harness.current_time(), Duration::ZERO);
    assert_eq!(harness.try_get_output("count"), Ok(Value::DInt(0)));
}

#[test]
fn harness_lifecycle_contract_from_sources_compiles_complete_ordered_set() {
    let harness =
        TestHarness::from_sources(&[helper_source(), counter_with_helper_source()]).unwrap();
    assert_eq!(harness.cycle_count(), 0);
    assert_eq!(harness.try_get_output("count"), Ok(Value::DInt(0)));
}

#[test]
fn harness_lifecycle_contract_compile_failure_publishes_no_harness() {
    let error = match TestHarness::from_source("PROGRAM Broken") {
        Ok(_) => panic!("invalid source must not build"),
        Err(error) => error,
    };
    assert!(!error.to_string().is_empty());
}

#[test]
fn harness_lifecycle_contract_runtime_accessors_share_one_runtime() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    let before = harness.runtime() as *const Runtime;
    let mutable = harness.runtime_mut() as *mut Runtime as *const Runtime;
    assert_eq!(before, mutable);
    harness
        .runtime_mut()
        .set_current_time(Duration::from_millis(7));
    assert_eq!(harness.runtime().current_time(), Duration::from_millis(7));
    assert_eq!(
        harness.into_runtime().current_time(),
        Duration::from_millis(7)
    );
}

#[test]
fn harness_lifecycle_contract_cycle_increments_once_and_reports_post_state() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    harness.advance_time(Duration::from_millis(5));
    let result = harness.cycle();
    assert_eq!(result.cycle_number, 1);
    assert_eq!(result.elapsed_time, Duration::from_millis(5));
    assert!(result.errors.is_empty());
    assert_eq!(harness.cycle_count(), 1);
    assert_eq!(harness.try_get_output("count"), Ok(Value::DInt(1)));
}

#[test]
fn harness_lifecycle_contract_failed_cycle_still_counts_once() {
    let mut harness = TestHarness::from_source(runtime_error_source()).unwrap();
    let result = harness.cycle();
    assert_eq!(result.cycle_number, 1);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(harness.cycle_count(), 1);
}

#[test]
fn harness_lifecycle_contract_run_cycles_zero_is_passive() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    assert!(harness.run_cycles(0).is_empty());
    assert_eq!(harness.cycle_count(), 0);
    assert_eq!(harness.try_get_output("count"), Ok(Value::DInt(0)));
}

#[test]
fn harness_lifecycle_contract_run_cycles_returns_exact_ordered_results() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    let results = harness.run_cycles(3);
    assert_eq!(
        results
            .iter()
            .map(|result| result.cycle_number)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(harness.cycle_count(), 3);
    assert_eq!(harness.try_get_output("count"), Ok(Value::DInt(3)));
}

#[test]
fn harness_lifecycle_contract_run_until_prechecks_current_runtime() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    let results = harness.run_until(|_| true);
    assert!(results.is_empty());
    assert_eq!(harness.cycle_count(), 0);
}

#[test]
fn harness_lifecycle_contract_run_until_returns_only_executed_cycles() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    let checks = Cell::new(0_u64);
    let results = harness.run_until_max(
        |_| {
            let current = checks.get();
            checks.set(current + 1);
            current == 3
        },
        3,
    );
    assert_eq!(results.len(), 3);
    assert_eq!(checks.get(), 4);
    assert_eq!(harness.cycle_count(), 3);
}

#[test]
fn harness_lifecycle_contract_run_until_max_panics_after_exact_budget() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        harness.run_until_max(|_| false, 2);
    }))
    .expect_err("budget exhaustion must panic");
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(message.contains("run_until exceeded 2 cycles"), "{message}");
    assert_eq!(harness.cycle_count(), 2);
}

#[test]
fn harness_lifecycle_contract_advance_time_saturates_at_signed_bounds() {
    let mut harness = TestHarness::from_source(counter_source()).unwrap();
    harness.advance_time(Duration::from_nanos(i64::MAX));
    harness.advance_time(Duration::from_nanos(1));
    assert_eq!(harness.current_time().as_nanos(), i64::MAX);
    harness
        .runtime_mut()
        .set_current_time(Duration::from_nanos(i64::MIN));
    harness.advance_time(Duration::from_nanos(-1));
    assert_eq!(harness.current_time().as_nanos(), i64::MIN);
}

#[test]
fn harness_lifecycle_contract_symbolic_boundary_is_fail_closed() {
    let mut harness = TestHarness::from_source(io_source()).unwrap();
    harness.try_set_input("input", Value::Bool(true)).unwrap();
    assert_eq!(harness.try_get_output("input"), Ok(Value::Bool(true)));
    assert_eq!(
        harness
            .try_set_input("missing", Value::Bool(true))
            .unwrap_err()
            .code(),
        "unresolved_name"
    );
    assert_eq!(harness.get_output("missing"), None);
}

#[test]
fn harness_lifecycle_contract_panicking_set_input_retains_boundary_message() {
    let mut harness = TestHarness::from_source(io_source()).unwrap();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        harness.set_input("missing", Value::Bool(true));
    }));
    assert!(panic.is_err());
    assert_eq!(harness.try_get_output("input"), Ok(Value::Bool(false)));
}

#[test]
fn harness_lifecycle_contract_direct_io_crosses_bound_cycle() {
    let mut harness = TestHarness::from_source(io_source()).unwrap();
    harness.bind_direct("input", "%IX0.0").unwrap();
    harness.bind_direct("output", "%QX0.0").unwrap();
    harness
        .set_direct_input("%IX0.0", Value::Bool(true))
        .unwrap();
    assert_eq!(
        harness.get_direct_output("%QX0.0").unwrap(),
        Value::Bool(false)
    );
    harness.cycle();
    assert_eq!(
        harness.get_direct_output("%QX0.0").unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn harness_lifecycle_contract_direct_io_error_ownership_is_stable() {
    let mut harness = TestHarness::from_source(io_source()).unwrap();
    assert!(matches!(
        harness.set_direct_input("bad", Value::Bool(true)),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
    assert!(matches!(
        harness.get_direct_output("bad"),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
    assert_eq!(
        harness.bind_direct("input", "bad").unwrap_err().code(),
        "internal_failure"
    );
}

#[test]
fn harness_lifecycle_contract_assert_eq_distinguishes_match_mismatch_and_missing() {
    let harness = TestHarness::from_source(counter_source()).unwrap();
    harness.assert_eq("count", Value::DInt(0));
    assert!(catch_unwind(AssertUnwindSafe(|| {
        harness.assert_eq("count", Value::DInt(1));
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| {
        harness.assert_eq("missing", Value::DInt(0));
    }))
    .is_err());
}

#[test]
fn harness_lifecycle_contract_reload_preserves_retain_time_and_cycle_count() {
    let mut harness = TestHarness::from_source(&retained_source(1)).unwrap();
    harness.try_set_input("counter", Value::Int(7)).unwrap();
    harness.advance_time(Duration::from_millis(25));
    harness.cycle();
    harness.reload_source(&retained_source(99)).unwrap();

    assert_eq!(harness.try_get_output("counter"), Ok(Value::Int(7)));
    assert_eq!(harness.current_time(), Duration::from_millis(25));
    assert_eq!(harness.cycle_count(), 1);
}

#[test]
fn harness_lifecycle_contract_reload_sources_preserves_state() {
    let mut harness = TestHarness::from_source(&retained_source(1)).unwrap();
    harness.try_set_input("counter", Value::Int(8)).unwrap();
    harness.advance_time(Duration::from_millis(10));
    harness.cycle();
    harness
        .reload_sources(&[helper_source(), retained_source(99).as_str()])
        .unwrap();

    assert_eq!(harness.try_get_output("counter"), Ok(Value::Int(8)));
    assert_eq!(harness.current_time(), Duration::from_millis(10));
    assert_eq!(harness.cycle_count(), 1);
}

#[test]
fn harness_lifecycle_contract_failed_reload_is_transactional() {
    let mut harness = TestHarness::from_source(&retained_source(1)).unwrap();
    harness.try_set_input("counter", Value::Int(8)).unwrap();
    harness.advance_time(Duration::from_millis(10));
    harness.cycle();
    assert!(harness.reload_source("PROGRAM Broken").is_err());

    assert_eq!(harness.try_get_output("counter"), Ok(Value::Int(8)));
    assert_eq!(harness.current_time(), Duration::from_millis(10));
    assert_eq!(harness.cycle_count(), 1);
}

#[test]
fn harness_lifecycle_contract_warm_and_cold_restart_apply_retain_policy() {
    let mut harness = TestHarness::from_source(&retained_source(1)).unwrap();
    harness.try_set_input("counter", Value::Int(7)).unwrap();
    harness.restart(crate::RestartMode::Warm).unwrap();
    assert_eq!(harness.try_get_output("counter"), Ok(Value::Int(7)));
    harness.restart(crate::RestartMode::Cold).unwrap();
    assert_eq!(harness.try_get_output("counter"), Ok(Value::Int(1)));
}
