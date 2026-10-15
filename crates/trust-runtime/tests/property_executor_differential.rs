//! Executor differential over bounded generated programs (scope H assurance, checklist A03).
//!
//! Each case is a generated Structured Text program (`support/st_program_gen.rs`) with one input
//! row per cycle. The program is compiled once, and its prepared module is instantiated three
//! times:
//!
//! - `stack`: `debug_active`, which forces the shared stack path;
//! - `register`: the register path with Tier-1 off;
//! - `tier1`: Tier-1 on with a hot threshold of 1, so every block specializes at once.
//!
//! Every cycle runs on all three arms. The `execute_cycle` result, the fault, the outputs and each
//! `Main` variable must agree; reals compare by bit pattern, so NaN and -0.0 are exact. After the
//! cycles each arm must show the executor it was meant to run.
//!
//! The seed is fixed, so a run replays the same programs. The default is 48 cases. For a larger
//! campaign run
//!
//! ```text
//! PROPTEST_CASES=20000 cargo test -p trust-runtime --test property_executor_differential
//! ```
//!
//! A failing case is saved in proptest's `proptest-regressions` store and replayed first.
//!
//! A program whose register arm records a fallback reason does not run on the register path.
//! Such cases are counted, not failed. The run fails only when more than half of the cases fall
//! back, and the message lists the reasons, so a generator that stops exercising the register path
//! cannot pass silently.

#[path = "support/differential_arms.rs"]
mod differential_arms;
#[path = "support/st_program_gen.rs"]
mod st_program_gen;

use differential_arms::{
    instantiate, main_value, main_variable_names, prepare, profiled, run_cycle, same_value,
    tier1_arm, ConfiguredServices,
};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, RngSeed};
use st_program_gen::{program_case, ProgramCase};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use trust_runtime_core::value::Duration;

/// Seed shared by both properties.
const SEED: u64 = 0x7472_7573_7445_5841;
/// Cases when `PROPTEST_CASES` is unset.
const DEFAULT_CASES: u32 = 48;

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_CASES)
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: cases(),
        rng_seed: RngSeed::Fixed(SEED),
        max_shrink_iters: 4096,
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource(
            "proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

/// Cases seen by `executors_agree_on_generated_programs`, and how many were ineligible.
static CASES_SEEN: AtomicUsize = AtomicUsize::new(0);
static INELIGIBLE: AtomicUsize = AtomicUsize::new(0);
/// Set once the eligibility gate has run.
static RATIO_CHECKED: AtomicBool = AtomicBool::new(false);
/// Fallback reasons of the ineligible cases, with the number of cases each reason caused.
static FALLBACK_REASONS: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());

proptest! {
    #![proptest_config(config())]

    /// Every generated program is accepted by the compiler and admitted as saved STBC.
    #[test]
    fn generated_programs_compile(case in program_case()) {
        if let Err(message) = prepare(&case.source) {
            prop_assert!(false, "{}\n{:?}", message, case);
        }
    }

    /// The stack, register and Tier-1 arms agree on every cycle of every generated program.
    #[test]
    fn executors_agree_on_generated_programs(case in program_case()) {
        match run_arms(&case) {
            Ok(fallbacks) => record_case(&fallbacks),
            Err(message) => prop_assert!(false, "{}", message),
        }
    }
}

/// Counts a passing case. `fallbacks` lists the reasons the register arm fell back, and is empty
/// when the register path ran the program. Once every configured case has passed, the test fails
/// if more than half of the cases fell back, because the generator must exercise the register
/// path; the message names the reasons.
fn record_case(fallbacks: &[String]) {
    let seen = CASES_SEEN.fetch_add(1, Ordering::SeqCst) + 1;
    if !fallbacks.is_empty() {
        INELIGIBLE.fetch_add(1, Ordering::SeqCst);
        let mut reasons = FALLBACK_REASONS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for reason in fallbacks {
            *reasons.entry(reason.clone()).or_insert(0) += 1;
        }
    }
    if seen >= cases() as usize && !RATIO_CHECKED.swap(true, Ordering::SeqCst) {
        let ineligible = INELIGIBLE.load(Ordering::SeqCst);
        let reasons = FALLBACK_REASONS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        eprintln!(
            "register path: {} of {seen} generated programs eligible; fallback reasons: {reasons:?}",
            seen - ineligible
        );
        assert!(
            ineligible * 2 <= seen,
            "{ineligible} of {seen} generated programs fell back from the register path; fallback reasons (cases): {reasons:?}"
        );
    }
}

/// Runs `case` on the three arms. Returns the fallback reasons the register arm recorded, which are
/// empty when the register path ran the program. The error names the first disagreement and the
/// program.
fn run_arms(case: &ProgramCase) -> Result<Vec<String>, String> {
    let prepared = prepare(&case.source)?;
    let stack_services = ConfiguredServices {
        stack: true,
        configuration: profiled(),
    };
    let register_services = ConfiguredServices {
        stack: false,
        configuration: profiled(),
    };
    let tier1_services = ConfiguredServices {
        stack: false,
        configuration: tier1_arm(1),
    };
    let mut stack = instantiate(&prepared, &stack_services)?;
    let mut register = instantiate(&prepared, &register_services)?;
    let mut tier1 = instantiate(&prepared, &tier1_services)?;
    let names = main_variable_names(&register)?;
    // Cycles that returned `Ok` on an arm. A cycle that faults does not complete its blocks, so
    // the profile counters below are only required once some cycle has completed.
    let mut register_completed = 0_usize;
    let mut tier1_completed = 0_usize;

    for (index, row) in case.inputs.iter().enumerate() {
        let cycle = index as i64 + 1;
        let time = Duration::from_millis(cycle * 10);
        let stack_result = run_cycle(&mut stack, row, time)?;
        let register_result = run_cycle(&mut register, row, time)?;
        let tier1_result = run_cycle(&mut tier1, row, time)?;
        register_completed += usize::from(register_result.is_ok());
        tier1_completed += usize::from(tier1_result.is_ok());
        if stack_result != register_result || stack_result != tier1_result {
            return Err(disagreement(
                case,
                cycle,
                &format!(
                    "execute_cycle result: stack={stack_result:?} register={register_result:?} tier1={tier1_result:?}"
                ),
            ));
        }
        if stack.fault() != register.fault() || stack.fault() != tier1.fault() {
            return Err(disagreement(
                case,
                cycle,
                &format!(
                    "fault: stack={:?} register={:?} tier1={:?}",
                    stack.fault(),
                    register.fault(),
                    tier1.fault()
                ),
            ));
        }
        if stack.outputs() != register.outputs() || stack.outputs() != tier1.outputs() {
            return Err(disagreement(
                case,
                cycle,
                &format!(
                    "outputs: stack={:?} register={:?} tier1={:?}",
                    stack.outputs(),
                    register.outputs(),
                    tier1.outputs()
                ),
            ));
        }
        for name in &names {
            let stack_value = main_value(&stack, name)?;
            let register_value = main_value(&register, name)?;
            let tier1_value = main_value(&tier1, name)?;
            if !same_value(&stack_value, &register_value) || !same_value(&stack_value, &tier1_value)
            {
                return Err(disagreement(
                    case,
                    cycle,
                    &format!(
                        "variable `{name}`: stack={stack_value:?} register={register_value:?} tier1={tier1_value:?}"
                    ),
                ));
            }
        }
    }

    // Executor checks after the cycles.
    let stack_profile = stack.register_profile_snapshot();
    let debug_fallback = stack_profile
        .fallback_reasons
        .iter()
        .any(|reason| reason.reason == "debug_mode" && reason.count > 0);
    if stack_profile.register_programs_executed != 0 || !debug_fallback {
        return Err(format!(
            "stack arm must run the shared path under a debug_mode fallback: {stack_profile:?}\nprogram:\n{}",
            case.source
        ));
    }
    let register_tier1 = register.tier1_snapshot();
    if register_tier1.enabled || register_tier1.block_executions != 0 {
        return Err(format!(
            "register arm must not run Tier-1: {register_tier1:?}\nprogram:\n{}",
            case.source
        ));
    }
    let register_profile = register.register_profile_snapshot();
    let fallbacks: Vec<String> = register_profile
        .fallback_reasons
        .iter()
        .map(|reason| reason.reason.clone())
        .collect();
    if fallbacks.is_empty() {
        if register_completed > 0 && register_profile.register_programs_executed == 0 {
            return Err(format!(
                "register arm completed cycles without a register execution: {register_profile:?}\nprogram:\n{}",
                case.source
            ));
        }
        if tier1_completed > 0 {
            let tier1_profile = tier1.register_profile_snapshot();
            let tier1_blocks = tier1.tier1_snapshot();
            if tier1_profile.register_programs_executed == 0 || tier1_blocks.block_executions == 0 {
                return Err(format!(
                    "tier1 arm completed cycles without a Tier-1 execution: {tier1_profile:?} {tier1_blocks:?}\nprogram:\n{}",
                    case.source
                ));
            }
        }
    }
    Ok(fallbacks)
}

fn disagreement(case: &ProgramCase, cycle: i64, detail: &str) -> String {
    format!(
        "executor arms disagree at cycle {cycle}: {detail}\nprogram:\n{}",
        case.source
    )
}
