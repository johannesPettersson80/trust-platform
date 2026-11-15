//! A Rust POU's `EN` and `ENO` are Structured Text's (specification 48 RU-30; specification 04
//! §EN/ENO): `EN := FALSE` skips its body, an enabled call starts `ENO` `TRUE`, and the body sets it
//! `FALSE` with `cx.set_eno(false)`.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// Specification 04's `SafeDiv` and a block that counts the calls it lets through.
const ENABLE: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// `num / den`, and 0.0 with ENO FALSE for a zero divisor, as specification 04's `SafeDiv`.
#[trust_function(name = \"SAFE_DIV\")]
#[budget(\"1us\")]
pub fn safe_div(cx: &mut Cx, num: f64, den: f64) -> Result<f64, Fault> {
    if den == 0.0 {
        cx.set_eno(false);
        return Ok(0.0);
    }
    checked::fdiv(cx, num, den)
}

/// Counts the calls it is opened for; a closed call does no work, ENO FALSE.
#[trust_fb(name = \"GATE\")]
#[budget(\"1us\")]
pub struct Gate {
    #[input]
    pub open: bool,
    #[output]
    pub passed: i16,
}

impl Body for Gate {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        if self.open {
            self.passed = checked::add(cx, self.passed, 1)?;
        } else {
            cx.set_eno(false);
        }
        Ok(())
    }
}
";

#[test]
fn en_skips_a_rust_pou_and_its_body_sets_eno() {
    // RU-30 and specification 04 §EN/ENO, one scan at 10 ms: SAFE_DIV(1, 4) is 0.25 with ENO TRUE;
    // SAFE_DIV(1, 0) commits its 0.0 with ENO FALSE, as the body set it; with EN FALSE the call
    // skips the body, gives LREAL's default 0.0 in place of the variable's 7.0, and ENO FALSE.
    // GATE opened counts 1 with ENO TRUE; closed, it counts nothing and sets ENO FALSE; with EN
    // FALSE its body does not run, its instance keeps the count 1 and ENO is FALSE. Catches an EN
    // that runs the body, an ENO the body's choice does not reach, and one an enabled call does
    // not start TRUE.
    let program = "PROGRAM Line
VAR
    quotient : LREAL;
    valid : BOOL;
    zero : LREAL := 5.0;
    zero_valid : BOOL := TRUE;
    skipped : LREAL := 7.0;
    skipped_valid : BOOL := TRUE;
    gate : GATE;
    opened : BOOL;
    closed : BOOL := TRUE;
    disabled : BOOL := TRUE;
END_VAR
quotient := SAFE_DIV(num := 1.0, den := 4.0, ENO => valid);
zero := SAFE_DIV(num := 1.0, den := 0.0, ENO => zero_valid);
skipped := SAFE_DIV(EN := FALSE, num := 1.0, den := 2.0, ENO => skipped_valid);
gate(open := TRUE, ENO => opened);
gate(open := FALSE, ENO => closed);
gate(EN := FALSE, open := TRUE, ENO => disabled);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";
    let root = custom_project(
        "rust-pous-enable",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", ENABLE)],
        program,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    let expected = [
        ("MAIN.QUOTIENT", Value::LReal(0.25)),
        ("MAIN.VALID", Value::Bool(true)),
        ("MAIN.ZERO", Value::LReal(0.0)),
        ("MAIN.ZERO_VALID", Value::Bool(false)),
        ("MAIN.SKIPPED", Value::LReal(0.0)),
        ("MAIN.SKIPPED_VALID", Value::Bool(false)),
        ("MAIN.GATE.PASSED", Value::Int(1)),
        ("MAIN.OPENED", Value::Bool(true)),
        ("MAIN.CLOSED", Value::Bool(false)),
        ("MAIN.DISABLED", Value::Bool(false)),
    ];
    for (name, value) in expected {
        assert_eq!(program.value(name), Some(value), "{name}");
    }
}
