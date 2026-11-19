//! A Rust POU passes the project's enumerations and types with named values through a call of
//! Structured Text and a held block, and the types the crate declares through bindings of their own
//! (specification 48 RU-25, RU-39, RU-40).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A crate that steps the enumeration MODE, brightens the type with named values LIGHT, reads the
/// default of the structure SETTING of both and holds a switcher of modes.
const ENUMERATIONS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"STEPPED\")]
#[budget(\"1us\")]
pub fn stepped(cx: &mut Cx) -> Result<i16, Fault> {
    let mode = st::NEXT_MODE(cx, st::MODE::default())?;
    Ok(mode as i16)
}

#[trust_function(name = \"GLOW\")]
#[budget(\"1us\")]
pub fn glow(cx: &mut Cx) -> Result<i16, Fault> {
    let light = st::BRIGHTER(cx, st::LIGHT::AMBER)?;
    let setting = st::SETTING::default();
    Ok(light.0 * 100 + setting.L.0 * 10 + setting.M as i16)
}

#[trust_fb(name = \"SELECTOR\")]
#[budget(\"2us\")]
pub struct Selector {
    #[input] pub stop: bool,
    #[output] pub have: i16,
    #[output] pub changes: i32,
    switcher: st::SWITCHER,
}

impl Body for Selector {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.switcher.input.WANT = if self.stop { st::MODE::STOP } else { st::MODE::RUN };
        self.switcher.call(cx)?;
        self.have = self.switcher.output.HAVE as i16;
        self.changes = self.switcher.output.CHANGES;
        Ok(())
    }
}
";

#[test]
fn enumerations_and_named_values_pass_through_calls_and_held_blocks() {
    // RU-39: `st::MODE` is a Rust enum of MODE's values, starting at its declared RUN = 5, so
    // NEXT_MODE gives STOP = 6; `st::LIGHT` holds every INT, so BRIGHTER(AMBER) = 2 + 10 = 12,
    // which no named value has, comes back as it is, and the default SETTING holds L = AMBER = 2,
    // its field's initial value, and M = RUN = 5, MODE's: GLOW = 12 x 100 + 2 x 10 + 5 = 1225. The
    // held SWITCHER's HAVE starts at RUN and takes STOP = 6 with one change, at the task's first
    // call at 0 ms. Catches a default at the first value (NEXT_MODE(IDLE) = RUN = 5), a type with
    // named values read as its default (GREEN = 3, GLOW 325), and a held block's enumeration
    // left in the generated cells.
    let root = custom_project(
        "rust-pous-structured-text-enumerations",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", ENUMERATIONS),
        ],
        "TYPE MODE : (IDLE, RUN := 5, STOP) := RUN;
END_TYPE

TYPE LIGHT : INT (RED := 1, AMBER := 2, GREEN := 3) := GREEN;
END_TYPE

TYPE SETTING :
STRUCT
    M : MODE;
    L : LIGHT := AMBER;
END_STRUCT
END_TYPE

FUNCTION NEXT_MODE : MODE
VAR_INPUT
    M : MODE;
END_VAR
CASE M OF
    IDLE: NEXT_MODE := RUN;
    RUN: NEXT_MODE := STOP;
ELSE
    NEXT_MODE := IDLE;
END_CASE
END_FUNCTION

FUNCTION BRIGHTER : LIGHT
VAR_INPUT
    L : LIGHT;
END_VAR
BRIGHTER := L + 10;
END_FUNCTION

FUNCTION_BLOCK SWITCHER
VAR_INPUT
    WANT : MODE;
END_VAR
VAR_OUTPUT
    HAVE : MODE;
    CHANGES : DINT;
END_VAR
IF WANT <> HAVE THEN
    HAVE := WANT;
    CHANGES := CHANGES + 1;
END_IF
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    stepped_value : INT;
    glow_code : INT;
    selector : SELECTOR;
END_VAR
stepped_value := STEPPED();
glow_code := GLOW();
selector(stop := TRUE);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
",
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the scan at 0 ms is its activation.
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.STEPPED_VALUE"), Some(Value::Int(6)));
    assert_eq!(program.value("MAIN.GLOW_CODE"), Some(Value::Int(1225)));
    assert_eq!(program.value("MAIN.SELECTOR.HAVE"), Some(Value::Int(6)));
    assert_eq!(program.value("MAIN.SELECTOR.CHANGES"), Some(Value::DInt(1)));
}

/// A crate whose function passes its own structure and enumeration to Structured Text through
/// their bindings.
const CRATE_TYPES: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_enum(name = \"LEVEL\")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Low = 0,
    High = 3,
}

#[trust_struct(name = \"SPAN\")]
#[derive(Clone, Copy)]
pub struct Span {
    pub low: f64,
    #[init(10.0)]
    pub high: f64,
}

#[trust_function(name = \"LEVEL_OF\")]
#[budget(\"1us\")]
pub fn level_of(cx: &mut Cx, value: f64) -> Result<i16, Fault> {
    let span = Span { low: 0.0, high: 8.0 };
    let level = st::CLASSIFY(cx, value, st::SPAN { low: span.low, high: span.high })?;
    let own = match level {
        st::LEVEL::Low => Level::Low,
        st::LEVEL::High => Level::High,
    };
    Ok(own as i16)
}

#[trust_function(name = \"DEFAULT_HIGH\")]
#[budget(\"1us\")]
pub fn default_high(_cx: &mut Cx) -> Result<f64, Fault> {
    Ok(st::SPAN::default().high)
}
";

#[test]
fn the_crates_own_types_have_bindings_of_their_own() {
    // RU-39, RU-40: a structure and an enumeration that the crate declares, SPAN and LEVEL, have
    // bindings in layer 1 too, `st::SPAN` and `st::LEVEL` beside the crate's `Span` and `Level`:
    // CLASSIFY(6.0, (0.0, 8.0)) is High, 6.0 > (0.0 + 8.0) / 2, = 3; CLASSIFY(2.0, ..) is Low = 0;
    // and `st::SPAN::default()` holds the crate's initial value of high, 10.0, at the task's first
    // call at 0 ms. Catches a call of a crate's type refused, a binding whose values differ from
    // the crate's declaration, and a default without the crate's initial value.
    let root = custom_project(
        "rust-pous-structured-text-crate-types",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", CRATE_TYPES),
        ],
        "FUNCTION CLASSIFY : LEVEL
VAR_INPUT
    V : LREAL;
    S : SPAN;
END_VAR
IF V > (S.low + S.high) / 2.0 THEN
    CLASSIFY := High;
ELSE
    CLASSIFY := Low;
END_IF
END_FUNCTION

PROGRAM Line
VAR
    above : INT;
    below : INT;
    high : LREAL;
END_VAR
above := LEVEL_OF(value := 6.0);
below := LEVEL_OF(value := 2.0);
high := DEFAULT_HIGH();
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
",
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the scan at 0 ms is its activation.
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.ABOVE"), Some(Value::Int(3)));
    assert_eq!(program.value("MAIN.BELOW"), Some(Value::Int(0)));
    assert_eq!(program.value("MAIN.HIGH"), Some(Value::LReal(10.0)));
}

/// A crate whose function passes a value of an enumeration that gives two names one value.
const REPEATED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"PHASE_RANK\")]
#[budget(\"1us\")]
pub fn phase_rank(cx: &mut Cx) -> Result<i16, Fault> {
    let lowest = st::PHASE::LOWEST;
    let same = lowest == st::PHASE::LOW;
    Ok(st::RANK(cx, lowest)? * 10 + i16::from(same))
}
";

#[test]
fn an_enumeration_that_repeats_a_value_is_a_struct_of_its_values() {
    // RU-39: PHASE gives LOW and LOWEST the value 1, which a Rust enum cannot hold, so its binding
    // is a struct of one value with each name as its constant, as a type with named values': LOWEST
    // equals LOW, and RANK(LOWEST) = 1, so PHASE_RANK = 1 x 10 + 1 = 11, at the task's first call
    // at 0 ms. Catches an enumeration refused for a repeated value, and constants of other values.
    let root = custom_project(
        "rust-pous-structured-text-repeated-value",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", REPEATED),
        ],
        "TYPE PHASE : (LOW := 1, LOWEST := 1, HIGH := 2);
END_TYPE

FUNCTION RANK : INT
VAR_INPUT
    P : PHASE;
END_VAR
IF P = LOWEST THEN
    RANK := 1;
ELSIF P = HIGH THEN
    RANK := 2;
ELSE
    RANK := 0;
END_IF
END_FUNCTION

PROGRAM Line
VAR
    rank_code : INT;
END_VAR
rank_code := PHASE_RANK();
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
",
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the scan at 0 ms is its activation.
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.RANK_CODE"), Some(Value::Int(11)));
}
