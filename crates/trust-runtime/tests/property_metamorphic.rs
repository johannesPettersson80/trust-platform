//! Metamorphic checks over generated programs (scope H assurance, checklist A04).
//!
//! Each property takes a generated program (`support/st_program_gen.rs`) and changes it in one way
//! that the specifications say does not change behaviour, or prepares the same artifact again. The
//! changed program must then show the same observable behaviour on the same input rows: the
//! `execute_cycle` result, the fault, the process outputs and every `Main` variable. Reals compare
//! by bit pattern.
//!
//! The transformations, and the specification that makes each one behaviour-preserving:
//!
//! 1. Positional arguments: `F0(a := x, b := y)` is printed as `F0(x, y)`. Actuals are evaluated
//!    once, left to right in source order, and a complete non-formal call binds them to the inputs
//!    in declaration order (`06-statements.md` section 3.1; `04-pou-declarations.md`, Function Call
//!    (IEC 6.6.1.7); `05-expressions.md` section 5.3.1). The generator prints named actuals in
//!    declaration order, so the evaluation order is the same in both forms.
//! 2. POU order: the functions are printed after `PROGRAM Main`, last declared first. Source order
//!    is not a visibility rule, and project assembly establishes every function's identity before
//!    any body is lowered, so a body may name a function declared later (`10-runtime-semantics.md`
//!    section 7.10). That section states the rule for source units; this program is one unit.
//! 3. Artifact round trip: the compiled module is encoded, decoded and encoded again. The bytes
//!    must not change, and the artifact prepared from them must run like the original
//!    (`12-bytecode.md` sections 7.5.1 and 7.5.1.5).
//! 4. Repeated preparation: the same bytes are prepared twice. The two preparations must report
//!    the same register lowering and Tier-1 preparation snapshot, and each must run like the other
//!    on the register arm and on the Tier-1 arm (`34-runtime-portability.md` HOST-04;
//!    `12-bytecode.md` sections 7.9 and 7.9.3).
//!
//! Transformations that change evaluation order, aliasing or numeric rounding are excluded. That
//! rules out reassociating arithmetic, swapping comparison operands, reordering statements and
//! changing constants.
//!
//! The seed is fixed, so a run replays the same programs. The default is 48 cases per property. For
//! a larger campaign run
//!
//! ```text
//! PROPTEST_CASES=20000 cargo test -p trust-runtime --test property_metamorphic
//! ```
//!
//! A failing case is saved in proptest's `proptest-regressions` store and replayed first.

#[path = "support/differential_arms.rs"]
mod differential_arms;
#[path = "support/st_program_gen.rs"]
mod st_program_gen;

use differential_arms::{
    admit, compile, instantiate, main_value, main_variable_names, prepare, profiled, run_cycle,
    same_value, tier1_arm, ConfiguredServices,
};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, RngSeed};
use st_program_gen::{program_case, PrintOptions, ProgramCase};
use trust_runtime::bytecode::BytecodeModule;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparedModule};

/// Seed shared by the four properties ("trustMET" in ASCII).
const SEED: u64 = 0x7472_7573_744D_4554;
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

proptest! {
    #![proptest_config(config())]

    /// Every call printed with positional arguments behaves as the named form does.
    #[test]
    fn positional_arguments_preserve_behaviour(case in program_case()) {
        let changed = case.print(PrintOptions {
            positional_arguments: true,
            ..PrintOptions::default()
        });
        if let Err(message) = register_runs_agree(&case.source, &changed, &case.inputs) {
            prop_assert!(false, "{}\n{:?}", message, case);
        }
    }

    /// Functions printed after `PROGRAM Main`, last declared first, behave as before.
    #[test]
    fn pou_order_preserves_behaviour(case in program_case()) {
        let changed = case.print(PrintOptions {
            functions_after_main: true,
            reverse_functions: true,
            ..PrintOptions::default()
        });
        if let Err(message) = register_runs_agree(&case.source, &changed, &case.inputs) {
            prop_assert!(false, "{}\n{:?}", message, case);
        }
    }

    /// The encode, decode and re-encode round trip keeps the bytes, and the artifact runs alike.
    #[test]
    fn artifact_round_trip_preserves_behaviour(case in program_case()) {
        if let Err(message) = round_trip_agrees(&case) {
            prop_assert!(false, "{}\n{:?}", message, case);
        }
    }

    /// Two preparations of the same bytes report the same snapshot and run alike.
    #[test]
    fn repeated_preparation_preserves_behaviour(case in program_case()) {
        if let Err(message) = repeated_preparation_agrees(&case) {
            prop_assert!(false, "{}\n{:?}", message, case);
        }
    }
}

/// Services for the register arm: telemetry on, Tier-1 off.
fn register_services() -> ConfiguredServices {
    ConfiguredServices {
        stack: false,
        configuration: profiled(),
    }
}

/// Services for the Tier-1 arm, which selects a block once it has been entered once.
fn tier1_services() -> ConfiguredServices {
    ConfiguredServices {
        stack: false,
        configuration: tier1_arm(1),
    }
}

/// Everything one cycle leaves behind: the `execute_cycle` outcome, the fault, the process outputs
/// and each `Main` variable, in name order.
struct Cycle {
    result: Result<(), RuntimeError>,
    fault: Option<RuntimeError>,
    outputs: Vec<u8>,
    variables: Vec<(String, Value)>,
}

/// Runs a prepared module over `inputs` on one arm, one cycle per row, and records every cycle.
fn record(
    prepared: &PreparedModule,
    services: &dyn ExecutionServices,
    inputs: &[[Value; 4]],
) -> Result<Vec<Cycle>, String> {
    let mut state = instantiate(prepared, services)?;
    let mut names = main_variable_names(&state)?;
    names.sort();
    let mut cycles = Vec::with_capacity(inputs.len());
    for (index, row) in inputs.iter().enumerate() {
        let time = Duration::from_millis((index as i64 + 1) * 10);
        let result = run_cycle(&mut state, row, time)?;
        let mut variables = Vec::with_capacity(names.len());
        for name in &names {
            variables.push((name.clone(), main_value(&state, name)?));
        }
        cycles.push(Cycle {
            result,
            fault: state.fault().cloned(),
            outputs: state.outputs().to_vec(),
            variables,
        });
    }
    Ok(cycles)
}

/// The first cycle where two runs differ, or `None` when they agree.
fn first_difference(expected: &[Cycle], actual: &[Cycle]) -> Option<String> {
    if expected.len() != actual.len() {
        return Some(format!(
            "{} cycles against {}",
            expected.len(),
            actual.len()
        ));
    }
    for (index, (left, right)) in expected.iter().zip(actual).enumerate() {
        let cycle = index + 1;
        if left.result != right.result {
            return Some(format!(
                "cycle {cycle}: execute_cycle result {:?} against {:?}",
                left.result, right.result
            ));
        }
        if left.fault != right.fault {
            return Some(format!(
                "cycle {cycle}: fault {:?} against {:?}",
                left.fault, right.fault
            ));
        }
        if left.outputs != right.outputs {
            return Some(format!(
                "cycle {cycle}: outputs {:?} against {:?}",
                left.outputs, right.outputs
            ));
        }
        if left.variables.len() != right.variables.len() {
            return Some(format!(
                "cycle {cycle}: {} Main variables against {}",
                left.variables.len(),
                right.variables.len()
            ));
        }
        for ((name, value), (other_name, other)) in left.variables.iter().zip(&right.variables) {
            if name != other_name || !same_value(value, other) {
                return Some(format!(
                    "cycle {cycle}: variable `{name}` {value:?} against `{other_name}` {other:?}"
                ));
            }
        }
    }
    None
}

/// Runs the original and the changed source on the register arm over the same inputs. The changed
/// program must compile, and every cycle must agree.
fn register_runs_agree(original: &str, changed: &str, inputs: &[[Value; 4]]) -> Result<(), String> {
    let services = register_services();
    let expected = record(&prepare(original)?, &services, inputs)?;
    let changed_module =
        prepare(changed).map_err(|error| format!("{error}\nchanged program:\n{changed}"))?;
    let actual = record(&changed_module, &services, inputs)?;
    match first_difference(&expected, &actual) {
        None => Ok(()),
        Some(difference) => Err(format!("{difference}\nchanged program:\n{changed}")),
    }
}

/// Encodes the program, decodes the bytes and encodes them again, and requires the same bytes. The
/// original artifact and the re-encoded one must then run alike on the register arm.
fn round_trip_agrees(case: &ProgramCase) -> Result<(), String> {
    let bytes = compile(&case.source)?;
    let decoded =
        BytecodeModule::decode(&bytes).map_err(|error| format!("decode failed: {error:?}"))?;
    let reencoded = decoded
        .encode()
        .map_err(|error| format!("re-encode failed: {error:?}"))?;
    if reencoded != bytes {
        return Err(format!(
            "re-encoded artifact differs from the original: {} bytes against {} bytes",
            reencoded.len(),
            bytes.len()
        ));
    }
    let services = register_services();
    let original = record(&admit(&bytes)?, &services, &case.inputs)?;
    let redecoded = record(&admit(&reencoded)?, &services, &case.inputs)?;
    match first_difference(&original, &redecoded) {
        None => Ok(()),
        Some(difference) => Err(format!("re-decoded artifact: {difference}")),
    }
}

/// Prepares the same bytes twice. The two preparations must report the same register lowering and
/// Tier-1 preparation snapshot, and each arm must run both preparations to the same cycles.
fn repeated_preparation_agrees(case: &ProgramCase) -> Result<(), String> {
    let bytes = compile(&case.source)?;
    let first = admit(&bytes)?;
    let second = admit(&bytes)?;
    let snapshot = first.register_preparation_snapshot();
    if snapshot.pou_count == 0 {
        return Err("the preparation considered no POU body".to_string());
    }
    let second_snapshot = second.register_preparation_snapshot();
    if snapshot != second_snapshot {
        return Err(format!(
            "preparation snapshots differ:\nfirst: {snapshot:?}\nsecond: {second_snapshot:?}"
        ));
    }
    for (arm, services) in [
        ("register", register_services()),
        ("tier1", tier1_services()),
    ] {
        let expected = record(&first, &services, &case.inputs)?;
        let actual = record(&second, &services, &case.inputs)?;
        if let Some(difference) = first_difference(&expected, &actual) {
            return Err(format!("{arm} arm, second preparation: {difference}"));
        }
    }
    Ok(())
}
