//! Independent-oracle cases for standard function blocks: timers (TON), counters (CTU, CTD,
//! CTUD and typed variants) and the rising-edge trigger (R_TRIG).
//!
//! Source: A13/A27 numeric and standard-library audit, section 6 (prioritized missing cases),
//! families J (timers), K (counters, excluding the counter edge-memory-under-reset row, which
//! is owned by a separate change) and L (edges). Authorities: IEC 61131-3 Ed.3 Table 44
//! (R_TRIG, note on cold restart), Table 45 (counters, PVmax and PVmin), Table 46 and
//! Figure 15 (timers), and specification 08 sections 3, 4 and 5 (including restart and
//! skipped-call rules).
//!
//! Expected values come from the audit oracle, not from the implementation. A failing case is
//! a finding: report it, do not change the expectation. The counter storage cases drive the
//! builtin execution directly on a `VariableStorage` instance because the typed counter
//! maxima cannot be preset from Structured Text.

use trust_runtime::harness::TestHarness;
use trust_runtime::memory::VariableStorage;
use trust_runtime::stdlib::fbs::{execute_builtin_in_storage, BuiltinFbKind};
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const TON_PROGRAM: &str = "PROGRAM Main\n\
    VAR t : TON; in_ton : BOOL := TRUE; q : BOOL; et : TIME; END_VAR\n\
    t(IN := in_ton, PT := T#10ms, Q => q, ET => et);\n\
    END_PROGRAM";

fn compile(source: &str) -> TestHarness {
    TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture must compile: {error}"))
}

fn assert_ton_restarts_from_zero(mode: RestartMode) {
    let mut harness = compile(TON_PROGRAM);
    harness.cycle();
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("et", Value::Time(Duration::from_millis(5)));
    harness.restart(mode).expect("restart");
    harness.advance_time(Duration::from_millis(3));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness.assert_eq("q", Value::Bool(false));
    harness.assert_eq("et", Value::Time(Duration::ZERO));
}

#[test]
fn oracle_j5_ton_input_falling_on_the_expiry_call_resets_elapsed_time() {
    // J5: spec 08 section 5 and IEC Figure 15b. IN is sampled FALSE on the call where the
    // preset would be reached, so Q stays FALSE and ET returns to zero.
    let mut harness = compile(TON_PROGRAM);
    harness.cycle();
    harness.set_input("in_ton", false);
    harness.advance_time(Duration::from_millis(10));
    harness.cycle();
    harness.assert_eq("q", Value::Bool(false));
    harness.assert_eq("et", Value::Time(Duration::ZERO));
}

#[test]
fn oracle_j15_warm_restart_reinitialises_non_retained_ton() {
    // J15: IEC_DECISIONS 2026-07-12 (restart) and spec 08 section 5. A warm restart clears
    // the non-retained timer; its first executed call contributes no elapsed time.
    assert_ton_restarts_from_zero(RestartMode::Warm);
}

#[test]
fn oracle_j15_cold_restart_reinitialises_non_retained_ton() {
    // J15 (cold restart): the same expectation as the warm restart.
    assert_ton_restarts_from_zero(RestartMode::Cold);
}

#[test]
fn oracle_j16_skipped_call_interval_is_included_in_next_executed_call() {
    // J16: IEC_DECISIONS 2026-07-12 and spec 08 section 5. Time passes while the block is not
    // called; the next executed call adds the whole interval: 6 ms skipped plus 2 ms = 8 ms.
    // The audit's `en` is named `run_call` here because EN is a reserved identifier in truST.
    let mut harness = compile(
        "PROGRAM Main\n\
         VAR t : TON; run_call : BOOL := TRUE; in_ton : BOOL := TRUE; q : BOOL; et : TIME; END_VAR\n\
         IF run_call THEN\n\
             t(IN := in_ton, PT := T#50ms, Q => q, ET => et);\n\
         END_IF;\n\
         END_PROGRAM",
    );
    harness.cycle();
    harness.set_input("run_call", false);
    harness.advance_time(Duration::from_millis(6));
    harness.cycle();
    harness.set_input("run_call", true);
    harness.advance_time(Duration::from_millis(2));
    harness.cycle();
    harness.assert_eq("et", Value::Time(Duration::from_millis(8)));
    harness.assert_eq("q", Value::Bool(false));
}

#[test]
fn oracle_k4_ctu_at_int_maximum_keeps_value_on_rising_edge() {
    // K4: spec 08 section 4 and IEC Table 45 (PVmax). At the INT maximum a counting edge keeps
    // CV at 32767; Q is TRUE because CV >= PV.
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("CTU");
    for (name, value) in [
        ("CU", Value::Bool(true)),
        ("R", Value::Bool(false)),
        ("PV", Value::Int(32_767)),
        ("Q", Value::Bool(false)),
        ("CV", Value::Int(32_767)),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }
    execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, BuiltinFbKind::Ctu)
        .expect("CTU at the INT maximum");
    assert_eq!(
        storage.get_instance_var(instance, "CV"),
        Some(&Value::Int(32_767))
    );
    assert_eq!(
        storage.get_instance_var(instance, "Q"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn oracle_k7_ctd_udint_saturates_at_zero_without_wrapping() {
    // K7: spec 08 section 4 ("unsigned down counters saturate at zero"). A count edge at zero
    // leaves CV at 0 and Q TRUE; it must not wrap to the UDINT maximum.
    let mut harness = compile(
        "PROGRAM Main\n\
         VAR c : CTD_UDINT; cd : BOOL := FALSE; ld : BOOL := TRUE; pv : UDINT := UDINT#0;\n\
             q : BOOL; cv : UDINT; END_VAR\n\
         c(CD := cd, LD := ld, PV := pv, Q => q, CV => cv);\n\
         END_PROGRAM",
    );
    harness.cycle();
    harness.set_input("ld", false);
    harness.set_input("cd", true);
    harness.cycle();
    harness.assert_eq("cv", Value::UDInt(0));
    harness.assert_eq("q", Value::Bool(true));
}

#[test]
fn oracle_k8_ctd_int_counts_below_zero_to_minus_one() {
    // K8: spec 08 section 4 and IEC Table 45 (PVmin). The signed down counter may go below
    // zero; CV = -1 and Q is TRUE because CV <= 0 (IEC Table 45 CTD rule).
    let mut harness = compile(
        "PROGRAM Main\n\
         VAR c : CTD; cd : BOOL := FALSE; ld : BOOL := TRUE; pv : INT := INT#0;\n\
             q : BOOL; cv : INT; END_VAR\n\
         c(CD := cd, LD := ld, PV := pv, Q => q, CV => cv);\n\
         END_PROGRAM",
    );
    harness.cycle();
    harness.set_input("ld", false);
    harness.set_input("cd", true);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(-1));
    harness.assert_eq("q", Value::Bool(true));
}

#[test]
fn oracle_k14_ctu_udint_at_type_maximum_keeps_value_on_rising_edge() {
    // K14: spec 08 section 4. The up counter saturates at the UDINT maximum.
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("CTU");
    for (name, value) in [
        ("CU", Value::Bool(true)),
        ("R", Value::Bool(false)),
        ("PV", Value::UDInt(u32::MAX)),
        ("Q", Value::Bool(false)),
        ("CV", Value::UDInt(u32::MAX)),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }
    execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, BuiltinFbKind::Ctu)
        .expect("CTU_UDINT at the type maximum");
    assert_eq!(
        storage.get_instance_var(instance, "CV"),
        Some(&Value::UDInt(4_294_967_295))
    );
    assert_eq!(
        storage.get_instance_var(instance, "Q"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn oracle_k14_ctud_ulint_up_count_at_type_maximum_saturates() {
    // K14 (CTUD_ULINT): spec 08 section 4. A counting edge at ULINT MAX keeps CV at MAX; QU
    // is TRUE because CV >= PV and QD is FALSE because CV is not zero.
    let mut storage = VariableStorage::default();
    let instance = storage.create_instance("CTUD");
    for (name, value) in [
        ("CU", Value::Bool(true)),
        ("CD", Value::Bool(false)),
        ("R", Value::Bool(false)),
        ("LD", Value::Bool(false)),
        ("PV", Value::ULInt(u64::MAX)),
        ("QU", Value::Bool(false)),
        ("QD", Value::Bool(false)),
        ("CV", Value::ULInt(u64::MAX)),
    ] {
        assert!(storage.set_instance_var(instance, name, value));
    }
    execute_builtin_in_storage(&mut storage, Duration::ZERO, instance, BuiltinFbKind::Ctud)
        .expect("CTUD_ULINT at the type maximum");
    assert_eq!(
        storage.get_instance_var(instance, "CV"),
        Some(&Value::ULInt(u64::MAX))
    );
    assert_eq!(
        storage.get_instance_var(instance, "QU"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        storage.get_instance_var(instance, "QD"),
        Some(&Value::Bool(false))
    );
}

#[test]
fn oracle_l7_r_trig_with_clk_high_on_first_execution_is_a_rising_edge() {
    // L7: IEC 61131-3 section 6.6.3.5.3 NOTE and spec 08 section 3. The cold-start edge memory
    // is FALSE, so CLK TRUE on the first execution is a rising edge; the next execution with
    // CLK still TRUE is not.
    let mut harness = compile(
        "PROGRAM Main\n\
         VAR r : R_TRIG; clk : BOOL := TRUE; q : BOOL; END_VAR\n\
         r(CLK := clk, Q => q);\n\
         END_PROGRAM",
    );
    harness.cycle();
    harness.assert_eq("q", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q", Value::Bool(false));
}
