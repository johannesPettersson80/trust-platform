//! A Rust POU's values of the project's Structured Text interfaces (specification 48 RU-25, RU-33,
//! RU-39): the interface's binding as a variable, its methods called through it, which reach the
//! method of the instance's class, `NULL` and equality, an interface reached through a held
//! block's method result, a held instance as an interface's value, and the refusal of an interface that a Rust
//! block implements.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{build_error, built, custom_project, manifest};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::{load, Build, Device, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A crate whose block calls `SCALED` through the interface it is given, and compares it with
/// another.
const SCALER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"SCALER\")]
#[budget(\"2us\")]
pub struct Scaler {
    #[input] pub speed: st::I_SPEED,
    #[input] pub other: st::I_SPEED,
    #[input] pub x: i16,
    #[output] pub y: i16,
    #[output] pub unset: bool,
    #[output] pub same: bool,
}

impl Body for Scaler {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.unset = self.speed.is_null();
        self.same = self.speed == self.other;
        if !self.unset {
            self.y = self.speed.SCALED(cx, self.x)?;
        }
        Ok(())
    }
}
";

/// The interface I_SPEED, with a method its base I_NAMED declares, and two Structured Text blocks
/// that implement it: DOUBLER scales by two, TRIPLER by three plus the calls it counted.
const I_SPEED: &str = "INTERFACE I_NAMED
METHOD KIND : INT
END_METHOD
END_INTERFACE

INTERFACE I_SPEED EXTENDS I_NAMED
METHOD SCALED : INT
VAR_INPUT
    X : INT;
END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK DOUBLER IMPLEMENTS I_SPEED
METHOD PUBLIC SCALED : INT
VAR_INPUT
    X : INT;
END_VAR
SCALED := X * 2;
END_METHOD
METHOD PUBLIC KIND : INT
KIND := 2;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRIPLER IMPLEMENTS I_SPEED
VAR
    calls : INT;
END_VAR
METHOD PUBLIC SCALED : INT
VAR_INPUT
    X : INT;
END_VAR
calls := calls + 1;
SCALED := X * 3 + calls;
END_METHOD
METHOD PUBLIC KIND : INT
KIND := 3;
END_METHOD
END_FUNCTION_BLOCK
";

/// I_SPEED, its blocks, `program` and a configuration that runs it as `Main` every 10 ms.
fn with_interface(program: &str) -> String {
    format!(
        "{I_SPEED}
{program}

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
"
    )
}

/// The project of the crate `lib` and the Structured Text `program`, built, or `None` where the
/// toolchain builds no Rust POU.
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
        &with_interface(program),
    );
    built(&root)
}

#[test]
fn a_rust_block_calls_methods_through_an_interface_it_holds() {
    // RU-33, RU-39, IEC §6.6.6.5 and §6.6.8, at the task's first call at 0 ms: BY_TWO is given
    // DOUBLER and scales 5 to 5 × 2 = 10; BY_THREE is given TRIPLER, whose own SCALED counts the
    // call on the instance it names, so 5 × 3 + 1 = 16 and TRIPLER.CALLS is 1; NONE is given
    // nothing, so its interface is NULL, the value a variable of an interface starts at, and it
    // calls nothing. Two values are equal when they name the same instance (BY_TWO's two DOUBLERs)
    // or are both NULL (NONE's), not for two instances (BY_THREE's). Catches a binding that is not
    // the interface's value, a call that reaches another class's method or a copy of the
    // instance, a NULL that is not the default, and equality by anything but the instance.
    let Some((built, device)) = build(
        "rust-pous-structured-text-interfaces",
        SCALER,
        "PROGRAM Line
VAR
    doubler : DOUBLER;
    tripler : TRIPLER;
    by_two : SCALER;
    by_three : SCALER;
    none : SCALER;
    two : INT;
    three : INT;
    unset : BOOL;
    same_two : BOOL;
    same_three : BOOL;
    same_none : BOOL;
END_VAR
by_two(speed := doubler, other := doubler, x := 5);
by_three(speed := tripler, other := doubler, x := 5);
none(x := 5);
two := by_two.y;
three := by_three.y;
unset := none.unset;
same_two := by_two.same;
same_three := by_three.same;
same_none := none.same;
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
    assert_eq!(program.value("MAIN.TWO"), Some(Value::Int(10)));
    assert_eq!(program.value("MAIN.THREE"), Some(Value::Int(16)));
    assert_eq!(program.value("MAIN.TRIPLER.CALLS"), Some(Value::Int(1)));
    assert_eq!(program.value("MAIN.UNSET"), Some(Value::Bool(true)));
    assert_eq!(program.value("MAIN.NONE.Y"), Some(Value::Int(0)));
    assert_eq!(program.value("MAIN.SAME_TWO"), Some(Value::Bool(true)));
    assert_eq!(program.value("MAIN.SAME_THREE"), Some(Value::Bool(false)));
    assert_eq!(program.value("MAIN.SAME_NONE"), Some(Value::Bool(true)));
}

/// A crate whose block calls `SCALED` and its base's `KIND` through whatever interface it is
/// given, NULL included.
const BLIND: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"BLIND\")]
#[budget(\"2us\")]
pub struct Blind {
    #[input] pub speed: st::I_SPEED,
    #[output] pub kind: i16,
    #[output] pub y: i16,
}

impl Body for Blind {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.kind = self.speed.KIND(cx)?;
        self.y = self.speed.SCALED(cx, 7)?;
        Ok(())
    }
}
";

#[test]
fn a_method_of_the_interfaces_base_and_a_call_through_null() {
    // RU-39, IEC §6.6.6.6: the binding holds the method KIND of I_NAMED, which I_SPEED extends,
    // so given TRIPLER the block reads KIND 3 and SCALED 7 × 3 + 1 = 22 at 0 ms. IEC §6.6.6.5.2,
    // specification 44 §6.5: a call through NULL ends the scan with `NullReference`, as Structured
    // Text's call through the interface does. Catches a base's method left out of the binding and a
    // NULL call that reads zero instead of ending the scan.
    let Some((built, device)) = build(
        "rust-pous-structured-text-interface-base",
        BLIND,
        "PROGRAM Line
VAR
    tripler : TRIPLER;
    blind : BLIND;
END_VAR
blind(speed := tripler);
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
    assert_eq!(program.value("MAIN.BLIND.KIND"), Some(Value::Int(3)));
    assert_eq!(program.value("MAIN.BLIND.Y"), Some(Value::Int(22)));

    let Some((built, device)) = build(
        "rust-pous-structured-text-interface-null",
        BLIND,
        "PROGRAM Line
VAR
    blind : BLIND;
END_VAR
blind();
END_PROGRAM",
    ) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the first scan runs it.
    let error = program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect_err("the call through NULL ends the scan");
    assert_eq!(error, RuntimeError::NullReference);
}

/// A crate whose block calls SCALED through the interface its held FACTORY's method gives, without
/// naming the interface.
const PICKER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"PICKER\")]
#[budget(\"2us\")]
pub struct Picker {
    #[input] pub fast: bool,
    #[output] pub y: i16,
    factory: st::FACTORY,
}

impl Body for Picker {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        let speed = self.factory.PICK(cx, self.fast)?;
        self.y = speed.SCALED(cx, 4)?;
        Ok(())
    }
}
";

#[test]
fn an_interface_a_held_blocks_method_returns_has_its_binding() {
    // RU-39: an interface that a held block's method passes has its binding without the crate
    // naming it: FACTORY's PICK gives its own TRIPLER for TRUE, so PICKER reads 4 × 3 + 1 = 13 at
    // 10 ms, and that TRIPLER counted the call. Catches the binding of an interface the crate does
    // not name written without the dispatchers of its methods, which layer 1 would then miss.
    let Some((built, device)) = build(
        "rust-pous-structured-text-interface-result",
        PICKER,
        "FUNCTION_BLOCK FACTORY
VAR
    slow : DOUBLER;
    fast : TRIPLER;
END_VAR
METHOD PUBLIC PICK : I_SPEED
VAR_INPUT
    FAST_ONE : BOOL;
END_VAR
IF FAST_ONE THEN
    PICK := fast;
ELSE
    PICK := slow;
END_IF
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    picker : PICKER;
END_VAR
picker(fast := TRUE);
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
    assert_eq!(program.value("MAIN.PICKER.Y"), Some(Value::Int(13)));
    assert_eq!(
        program.value("MAIN.PICKER.FACTORY.FAST.CALLS"),
        Some(Value::Int(1))
    );
}

/// A crate whose block holds a TRIPLER, calls SCALED through the interface value of it, and gives
/// that value out.
const HOLDER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"HOLDER\")]
#[budget(\"2us\")]
pub struct Holder {
    #[output] pub speed: st::I_SPEED,
    #[output] pub y: i16,
    tripler: st::TRIPLER,
}

impl Body for Holder {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.speed = st::I_SPEED::from(&self.tripler);
        self.y = self.speed.SCALED(cx, 2)?;
        Ok(())
    }
}
";

#[test]
fn a_held_instance_is_a_value_of_the_interface_it_implements() {
    // RU-39, IEC §6.6.6.5.1, at the task's first call at 0 ms: HOLDER's own TRIPLER is a value of
    // I_SPEED, the reference to it, so SCALED through it reaches that TRIPLER, 2 × 3 + 1 = 7; the
    // program then calls SCALED through HOLDER's output, which names the same instance, 1 × 3 + 2
    // = 5, its second call. Catches a conversion that names another instance or a copy, and an
    // interface value that Structured Text cannot call through.
    let Some((built, device)) = build(
        "rust-pous-structured-text-interface-of-held",
        HOLDER,
        "PROGRAM Line
VAR
    holder : HOLDER;
    again : INT;
END_VAR
holder();
again := holder.speed.SCALED(X := 1);
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
    assert_eq!(program.value("MAIN.HOLDER.Y"), Some(Value::Int(7)));
    assert_eq!(program.value("MAIN.AGAIN"), Some(Value::Int(5)));
    assert_eq!(
        program.value("MAIN.HOLDER.TRIPLER.CALLS"),
        Some(Value::Int(2))
    );
}

/// A crate whose block HAND implements I_SPEED in Rust, beside SCALER, which calls through it.
fn rust_implementation() -> String {
    format!(
        "{SCALER}
#[trust_fb(name = \"HAND\", implements = \"I_SPEED\")]
#[budget(\"1us\")]
pub struct Hand {{
    #[output] pub last: i16,
}}

impl Body for Hand {{
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {{
        Ok(())
    }}
}}

impl Hand {{
    #[trust_method]
    #[budget(\"1us\")]
    pub fn scaled(&mut self, _cx: &mut Cx, x: i16) -> Result<i16, Fault> {{
        self.last = x;
        Ok(x)
    }}

    #[trust_method]
    #[budget(\"1us\")]
    pub fn kind(&mut self, _cx: &mut Cx) -> Result<i16, Fault> {{
        Ok(9)
    }}
}}
"
    )
}

#[test]
fn an_interface_a_rust_block_implements_has_no_binding() {
    // RU-39, RU-40: the binding's call of SCALED can reach HAND's, a Rust method, and layer 1,
    // compiled before the crate, cannot call it, so the build refuses the binding with the chain
    // from the crate's use, on every toolchain. Catches a layer 1 that calls the crate, which cargo
    // could not build.
    let root = custom_project(
        "rust-pous-structured-text-interface-rust",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", &rust_implementation()),
        ],
        &with_interface(
            "PROGRAM Line
VAR
    hand : HAND;
    scaler : SCALER;
END_VAR
scaler(speed := hand, x := 1);
END_PROGRAM",
        ),
    );
    // The generator refuses it while it writes the layers, before cargo runs (specification 48
    // §9.1, steps 3 and 4), so a toolchain without cargo refuses it the same.
    let (error, _) = build_error(&root);
    assert!(
        error.contains("st::I_SPEED at rust/src/lib.rs:"),
        "the refusal names the crate's use: {error}"
    );
    assert!(
        error.contains("HAND.")
            && error
                .contains("(Rust): Structured Text that Rust calls is compiled before the crate"),
        "the refusal names the Rust method and the rule: {error}"
    );
}
