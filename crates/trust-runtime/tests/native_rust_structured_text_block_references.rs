//! A Rust POU's references to function block instances of the project's Structured Text
//! (specification 48 RU-25, RU-39): `Ref<st::X>`, whose `get` gives the block's binding naming the
//! instance; its call and its methods reach the body and the methods of the instance's own class,
//! and `NULL` ends the scan.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::{load, Build, Device, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A crate whose block calls the COUNTER its reference names, after its RESET when asked.
const DRIVER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"DRIVER\")]
#[budget(\"2us\")]
pub struct Driver {
    #[input] pub target: Ref<st::COUNTER>,
    #[input] pub reset: bool,
    #[output] pub seen: i16,
    #[output] pub unset: bool,
}

impl Body for Driver {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.unset = self.target.is_null();
        let mut counter = self.target.get(cx)?;
        if self.reset {
            counter.RESET(cx)?;
        }
        counter.input.INC = 5;
        counter.call(cx)?;
        self.seen = counter.output.N;
        Ok(())
    }
}
";

/// COUNTER, which counts by its INC and whose RESET sets it to 0, and DOUBLE_COUNTER, derived from
/// it, which counts by twice its INC and whose RESET sets it to 100.
const COUNTERS: &str = "FUNCTION_BLOCK COUNTER
VAR_INPUT
    INC : INT;
END_VAR
VAR_OUTPUT
    N : INT;
END_VAR
N := N + INC;
METHOD PUBLIC RESET
N := 0;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK DOUBLE_COUNTER EXTENDS COUNTER
N := N + 2 * INC;
METHOD PUBLIC OVERRIDE RESET
N := 100;
END_METHOD
END_FUNCTION_BLOCK
";

/// The project of the crate `lib`, COUNTERS and `program`, run as `Main` every 10 ms, built, or
/// `None` where the toolchain builds no Rust POU.
fn build(name: &str, lib: &str, program: &str) -> Option<(Build, Device)> {
    let root = custom_project(
        name,
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", lib),
        ],
        &format!(
            "{COUNTERS}
{program}

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
"
        ),
    );
    built(&root)
}

#[test]
fn a_rust_block_calls_the_block_its_reference_names() {
    // RU-39, IEC §6.6.8, at the task's first call at 0 ms: BY_PLAIN's reference names PLAIN, a
    // COUNTER, whose body counts 0 + 5 = 5; BY_DOUBLED's names DOUBLED, a DOUBLE_COUNTER taken as a
    // COUNTER, whose own body counts 0 + 2 × 5 = 10. At 20 ms both RESET first: COUNTER's sets 0 and
    // DOUBLE_COUNTER's own sets 100, so PLAIN counts 0 + 5 = 5 and DOUBLED 100 + 10 = 110. The
    // calls change the instances themselves, as SEEN and the programs' counters agree. Catches a
    // reference read as a copy of the instance, and a call or a method bound to COUNTER's where the
    // instance is a DOUBLE_COUNTER.
    let Some((built, device)) = build(
        "rust-pous-structured-text-block-references",
        DRIVER,
        "PROGRAM Line
VAR
    plain : COUNTER;
    doubled : DOUBLE_COUNTER;
    by_plain : DRIVER;
    by_doubled : DRIVER;
    calls : INT;
END_VAR
calls := calls + 1;
by_plain(target := REF(plain), reset := calls > 1);
by_doubled(target := REF(doubled), reset := calls > 1);
END_PROGRAM",
    ) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the scan at 0 ms is its activation.
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.PLAIN.N"), Some(Value::Int(5)));
    assert_eq!(program.value("MAIN.BY_PLAIN.SEEN"), Some(Value::Int(5)));
    assert_eq!(program.value("MAIN.DOUBLED.N"), Some(Value::Int(10)));
    assert_eq!(program.value("MAIN.BY_DOUBLED.SEEN"), Some(Value::Int(10)));
    assert_eq!(
        program.value("MAIN.BY_PLAIN.UNSET"),
        Some(Value::Bool(false))
    );
    program
        .scan(Duration::from_millis(20), &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.PLAIN.N"), Some(Value::Int(5)));
    assert_eq!(program.value("MAIN.DOUBLED.N"), Some(Value::Int(110)));
    assert_eq!(program.value("MAIN.BY_DOUBLED.SEEN"), Some(Value::Int(110)));
}

#[test]
fn a_null_reference_to_a_block_ends_the_scan() {
    // RU-39, specification 44 §6.5: `get` of a reference that is NULL, the value it starts at,
    // ends the scan with `NullReference`, as Structured Text's dereference does; UNSET read TRUE
    // before. Catches a NULL read as an instance at offset 0.
    let Some((built, device)) = build(
        "rust-pous-structured-text-block-reference-null",
        DRIVER,
        "PROGRAM Line
VAR
    driver : DRIVER;
END_VAR
driver();
END_PROGRAM",
    ) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the first scan runs it.
    let error = program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect_err("the NULL reference ends the scan");
    assert_eq!(error, RuntimeError::NullReference);
}

/// A crate whose block counts through the COUNTER its held FACTORY's method gives a reference to,
/// without naming COUNTER.
const PICKER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"PICKER\")]
#[budget(\"2us\")]
pub struct Picker {
    #[output] pub seen: i16,
    factory: st::FACTORY,
}

impl Body for Picker {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        let mut counter = self.factory.FIRST(cx)?.get(cx)?;
        counter.input.INC = 3;
        counter.call(cx)?;
        self.seen = counter.output.N;
        Ok(())
    }
}
";

#[test]
fn a_reference_a_held_blocks_method_returns_has_its_blocks_binding() {
    // RU-39: a reference to a block that a held block's method returns has the block's binding
    // without the crate naming the block: FACTORY's FIRST gives a reference to its own INNER, a
    // DOUBLE_COUNTER taken as a COUNTER, so PICKER counts it by 2 × 3 = 6 at 0 ms. Catches the
    // binding of a block reached only through a call's result left unwritten, or without the body
    // dispatcher it calls.
    let Some((built, device)) = build(
        "rust-pous-structured-text-block-reference-result",
        PICKER,
        "FUNCTION_BLOCK FACTORY
VAR
    inner : DOUBLE_COUNTER;
END_VAR
METHOD PUBLIC FIRST : REF_TO COUNTER
FIRST := REF(inner);
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    picker : PICKER;
END_VAR
picker();
END_PROGRAM",
    ) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the scan at 0 ms is its activation.
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.PICKER.SEEN"), Some(Value::Int(6)));
    assert_eq!(
        program.value("MAIN.PICKER.FACTORY.INNER.N"),
        Some(Value::Int(6))
    );
}
