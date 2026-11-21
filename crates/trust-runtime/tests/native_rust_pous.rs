//! A project with Rust POUs builds into one program image with its crate, and its Structured Text
//! calls them as it calls its own POUs (specification 48 §6.3, §9): a Rust function block runs its
//! body on its instance's variables, a Rust function returns its result, and a fault a Rust POU
//! returns ends the scan with its code, leaving the instance as it was (RU-14, RU-28).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use std::path::{Path, PathBuf};

use rust_pous_support::{build_error, built, custom_project, manifest, measure_here, sources};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::rust_pous::{write_declarations, Written, DECLARATIONS_FILE};
use trust_runtime::native::{build_project, load, Build, NoHooks, Options, Target, Toolchain};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

/// The crate: RU-22's FILTER_EMA and MAX_LIMITED, a counter of the calls its level is above 100 in,
/// and a block whose count faults on overflow as Structured Text's does (RU-15).
const CRATE: &str = r#"#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Exponential moving average of one input.
#[trust_fb(name = "FILTER_EMA")]
#[budget("15us")]
pub struct FilterEma {
    #[input] pub x: f64,
    #[input] #[init(0.1)] pub alpha: f64,
    #[output] pub y: f64,
    primed: bool,
}

impl Body for FilterEma {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        if self.primed {
            self.y += self.alpha * (self.x - self.y);
        } else {
            self.y = self.x;
            self.primed = true;
        }
        Ok(())
    }
}

/// The larger of two inputs, limited to a maximum.
#[trust_function(name = "MAX_LIMITED")]
#[budget("2us")]
pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
    Ok(a.max(b).min(limit))
}

/// Counts the calls its level is above 100 in.
#[trust_fb(name = "HIGH_COUNTER")]
#[budget("1us")]
pub struct HighCounter {
    #[input] pub level: i16,
    #[output] pub high: bool,
    #[output] pub count: i16,
}

impl Body for HighCounter {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.high = self.level > 100;
        if self.high {
            self.count = checked::add(cx, self.count, 1)?;
        }
        Ok(())
    }
}

/// Marks its call, then counts from the largest INT: its first count overflows.
#[trust_fb(name = "OVERFLOWING")]
#[budget("1us")]
pub struct Overflowing {
    #[output] pub called: bool,
    #[output] #[init(32767)] pub count: i16,
}

impl Body for Overflowing {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.called = true;
        self.count = checked::add(cx, self.count, 1)?;
        Ok(())
    }
}
"#;

/// A program that calls the three POUs once per 10 ms, its level rising by 2.0 at each call.
const LINE: &str = "PROGRAM Line
VAR
    filter : FILTER_EMA;
    counter : HIGH_COUNTER;
    level : LREAL;
    smoothed : LREAL;
    limited : DINT;
    high : BOOL;
    count : INT;
END_VAR
level := level + 2.0;
filter(x := level, alpha := 0.5);
smoothed := filter.y;
limited := MAX_LIMITED(a := 3, b := 7, limit := 5);
counter(level := 200);
high := counter.high;
count := counter.count;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// A project folder with the crate, the program `program` and the declarations the crate declares.
fn project(name: &str, program: &str) -> PathBuf {
    let root = trust_test_support::unique_temp_path(name);
    let manifest = format!(
        "[package]\nname = \"rust-line\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
         [dependencies]\ntrust-plc = \"={}\"\n",
        env!("CARGO_PKG_VERSION")
    );
    for (path, text) in [
        ("rust/Cargo.toml", manifest.as_str()),
        ("rust/src/lib.rs", CRATE),
        ("src/main.st", program),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    assert_eq!(write_declarations(&root), Ok(Written::Wrote(4)));
    root
}

#[test]
fn structured_text_calls_rust_pous_as_its_own() {
    // Catches a build that leaves the Rust POUs out or calls them with their variables in other
    // places: three calls at 0, 10 and 20 ms (the task is due at its start, specification 10 §4.3).
    // The level is 2, 4, 6; the filter primes with 2, then moves by half the difference: 2 +
    // 0.5 × (4 − 2) = 3, then 3 + 0.5 × (6 − 3) = 4.5. MAX_LIMITED(3, 7, 5) is min(max(3, 7), 5) = 5.
    // The counter's level 200 is above 100 at each call: high, and a count of 3.
    let root = project("rust-pous-line", LINE);
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("MAIN.LEVEL"), Some(Value::LReal(6.0)));
    assert_eq!(program.value("MAIN.SMOOTHED"), Some(Value::LReal(4.5)));
    assert_eq!(program.value("MAIN.LIMITED"), Some(Value::DInt(5)));
    assert_eq!(program.value("MAIN.HIGH"), Some(Value::Bool(true)));
    assert_eq!(program.value("MAIN.COUNT"), Some(Value::Int(3)));
    // The filter's own variables lie in its instance, which the symbol table names (RU-27).
    assert_eq!(program.value("MAIN.FILTER.ALPHA"), Some(Value::LReal(0.5)));
    assert_eq!(program.value("MAIN.FILTER.PRIMED"), Some(Value::Bool(true)));
}

/// A program that calls OVERFLOWING, then copies its mark.
const OVERFLOWING: &str = "PROGRAM Line
VAR
    block : OVERFLOWING;
    called : BOOL;
END_VAR
block();
called := block.called;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_rust_pous_fault_ends_the_scan_and_leaves_its_instance_as_it_was() {
    // Catches a fault that the glue swallows or that keeps what the body changed before it: RU-14
    // ends the scan with the fault's code, `Overflow` for 32,767 + 1 (RU-15), and RU-28 leaves the
    // instance's variables as the area held them: `called` stays FALSE although the body set it,
    // the count stays 32,767, and the statement after the call never runs.
    let root = project("rust-pous-overflow", OVERFLOWING);
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the overflow ends the scan");
    assert_eq!(error, RuntimeError::Overflow);
    assert_eq!(program.value("MAIN.BLOCK.CALLED"), Some(Value::Bool(false)));
    assert_eq!(program.value("MAIN.BLOCK.COUNT"), Some(Value::Int(32767)));
    assert_eq!(program.value("MAIN.CALLED"), Some(Value::Bool(false)));
}

#[test]
fn a_project_whose_declarations_are_out_of_date_does_not_build() {
    // RU-24: the build refuses committed declarations that are not what the crate declares, naming
    // the command that writes them, before it compiles anything.
    let root = project("rust-pous-stale", LINE);
    std::fs::write(
        root.join(DECLARATIONS_FILE),
        "(* GENERATED by trust-runtime from rust/src/lib.rs - do not edit *)\n",
    )
    .expect("the stale file");
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let error = build_project(
        &root,
        &sources(&root),
        &[],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .err()
    .expect("the build is refused")
    .to_string();
    assert!(
        error.contains("trust-dev rust declarations") && error.contains(DECLARATIONS_FILE),
        "{error}"
    );
}

/// A program that calls `PASS_ON`.
const PASS_ON_PROGRAM: &str =
    "PROGRAM Line\nVAR\n    result : DINT;\nEND_VAR\nresult := PASS_ON(a := 1);\nEND_PROGRAM\n";

/// A crate root with the function `PASS_ON`, after the attributes `head`.
fn root(head: &str) -> String {
    format!(
        "{head}use trust_plc::prelude::*;\n\n#[trust_function(name = \"PASS_ON\")]\n#[budget(\"1us\")]\n\
         pub fn pass_on(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {{\n    Ok(a)\n}}\n"
    )
}

#[test]
fn a_crate_that_breaks_its_rules_is_refused_before_it_compiles() {
    // RU-08: a crate whose root does not forbid unsafe code is refused, naming the line to write,
    // whatever the toolchain, since nothing has compiled yet.
    let root = custom_project(
        "rust-pous-rules-root",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &root("#![cfg_attr(not(test), no_std)]\n\n")),
        ],
        PASS_ON_PROGRAM,
    );
    let (error, _) = build_error(&root);
    assert!(
        error.contains("#![forbid(unsafe_code)]") && error.contains("RU-08"),
        "{error}"
    );
}

#[test]
fn a_dependency_with_unsafe_code_or_a_build_script_is_refused() {
    // RU-09 and RU-20: a crate the project's crate builds with holds no unsafe code and has no
    // build script; the build names each and where, before it compiles them. A toolchain without
    // cargo refuses the build for that reason first.
    let keep = "#![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n\n";
    let root = custom_project(
        "rust-pous-rules-graph",
        &[
            (
                "Cargo.toml",
                &manifest(
                    "fast = { path = \"vendor/fast\" }\nscripted = { path = \"vendor/scripted\" }\n",
                ),
            ),
            ("src/lib.rs", &root(keep)),
            (
                "vendor/fast/Cargo.toml",
                "[package]\nname = \"fast\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "vendor/fast/src/lib.rs",
                "#![no_std]\npub fn read(p: *const u8) -> u8 {\n    unsafe { *p }\n}\n",
            ),
            (
                "vendor/scripted/Cargo.toml",
                "[package]\nname = \"scripted\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            ("vendor/scripted/build.rs", "fn main() {}\n"),
            ("vendor/scripted/src/lib.rs", "#![no_std]\n"),
        ],
        PASS_ON_PROGRAM,
    );
    let (error, toolchain) = build_error(&root);
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return;
    }
    let lines: Vec<&str> = error.lines().collect();
    assert_eq!(lines.len(), 2, "{error}");
    assert!(
        lines[0].starts_with("fast 0.1.0 holds `unsafe` code at ")
            && lines[0].contains("lib.rs:3")
            && lines[0].contains("RU-20"),
        "{error}"
    );
    assert!(
        lines[1].starts_with("scripted 0.1.0 has a build script") && lines[1].contains("RU-09"),
        "{error}"
    );
}

/// `path` named relative to this process's folder, as `trust-runtime build --project .` names a
/// project.
#[cfg(unix)]
fn relative(path: &Path) -> PathBuf {
    let here = std::env::current_dir().expect("this process's folder");
    let mut here_parts = here.components().peekable();
    let mut path_parts = path.components().peekable();
    while let (Some(left), Some(right)) = (here_parts.peek(), path_parts.peek()) {
        if left != right {
            break;
        }
        here_parts.next();
        path_parts.next();
    }
    let mut relative: PathBuf = here_parts.map(|_| "..").collect();
    relative.extend(path_parts);
    relative
}

#[cfg(unix)]
#[test]
fn a_project_named_by_a_relative_path_builds() {
    // Catches a build that hands cargo the project's folder as it is named: `trust-runtime build
    // --project .` names it `.`, and cargo, which runs in the build's folder, looked for the crate
    // below that folder (found by the C6 case, 15 November 2026). Named relative to this process's
    // folder, the project and its build folder build as they do named in full.
    let root = project("rust-pous-relative", LINE);
    let named = relative(&root);
    assert!(named.is_relative(), "{}", named.display());
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let result = build_project(
        &named,
        &sources(&root),
        &[],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        ConversionProfile::Iec,
        &toolchain,
        &named.join("build/native"),
    );
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        let error = result.err().expect("the build is refused").to_string();
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return;
    }
    result.unwrap_or_else(|error| panic!("the project builds named {}: {error}", named.display()));
}

/// A function that refuses a level above 100 with a fault of its own.
const CHECKED_LEVEL: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"CHECKED_LEVEL\")]
#[budget(\"1us\")]
pub fn checked_level(cx: &mut Cx, level: i32) -> Result<i32, Fault> {
    if level > 100 {
        return Err(rust_fault(cx, 7, i64::from(level), 100));
    }
    Ok(level)
}
";

/// A program that calls CHECKED_LEVEL with 150 in a 10 ms task.
const CHECKED_PROGRAM: &str = "PROGRAM Line
VAR
    result : DINT := -1;
END_VAR
result := CHECKED_LEVEL(level := 150);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_rust_pous_own_fault_ends_the_scan_with_its_numbers() {
    // Catches a Rust POU's own fault reported as another, or without the numbers it chose: RU-14
    // gives it the code RustFault (specification 44 Appendix A, 20) with the POU's number 7 and its
    // detail 150 and 100, whose stable code is `runtime_rust_fault`; the assignment it ended never
    // stores, so the result keeps -1.
    let root = custom_project(
        "rust-pous-rust-fault",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", CHECKED_LEVEL)],
        CHECKED_PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the fault ends the scan");
    assert_eq!(
        error,
        RuntimeError::RustFault {
            number: 7,
            first: 150,
            second: 100
        }
    );
    assert_eq!(error.stable_code().as_str(), "runtime_rust_fault");
    assert_eq!(program.value("MAIN.RESULT"), Some(Value::DInt(-1)));
}

/// A block that divides by its input and a function that indexes a table by its input.
const PANICKING: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"RATIO\")]
#[budget(\"1us\")]
pub struct Ratio {
    #[input]
    pub num: i32,
    #[input]
    pub den: i32,
    #[output]
    pub q: i32,
}

impl Body for Ratio {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.q = self.num / self.den;
        Ok(())
    }
}

#[trust_function(name = \"PICK\")]
#[budget(\"1us\")]
pub fn pick(_cx: &mut Cx, index: i32) -> Result<i32, Fault> {
    let table = [10, 20, 30];
    Ok(table[index as usize])
}
";

/// A program that calls RATIO and PICK with values that change at each scan, so that the link
/// cannot prove their checks away.
const PANICKING_PROGRAM: &str = "PROGRAM Line
VAR
    ratio : RATIO;
    scans : DINT;
    picked : DINT;
END_VAR
scans := scans + 1;
ratio(num := 10, den := scans);
picked := PICK(index := scans);
END_PROGRAM
";

#[test]
fn a_rust_pou_that_can_panic_is_named_with_the_form_to_use() {
    // RU-14 and RU-45: a Rust POU never panics; the build names each POU whose code can, where it is
    // declared, with what panics and the form that returns a fault instead: `/` on integers can
    // divide by zero (checked::div), an index can leave the table (`get`).
    let root = custom_project(
        "rust-pous-panics",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", PANICKING)],
        PANICKING_PROGRAM,
    );
    let (error, toolchain) = build_error(&root);
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return;
    }
    assert!(error.contains("the module can panic"), "{error}");
    let named = |pou: &str, operation: &str, form: &str| {
        error.lines().any(|line| {
            line.contains(&format!("{pou} can panic: {operation}")) && line.contains(form)
        })
    };
    assert!(
        named(
            "the Rust POU RATIO (rust/src/lib.rs:8)",
            "an integer division by zero",
            "trust_plc::checked::div"
        ),
        "{error}"
    );
    assert!(
        named(
            "the Rust POU PICK (rust/src/lib.rs:26)",
            "an index outside an array or a slice",
            "`get(index)`"
        ),
        "{error}"
    );
}

/// A block whose budget is 4 ms and a function whose budget is 1 µs.
const TIMED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"SLOW\")]
#[budget(\"4ms\")]
pub struct Slow;

impl Body for Slow {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[trust_function(name = \"QUICK\")]
#[budget(\"1us\")]
pub fn quick(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    Ok(a.wrapping_add(1))
}
";

/// A 10 ms task that calls SLOW five times and QUICK in a WHILE loop.
const TIMED_PROGRAM: &str = "PROGRAM Line
VAR
    slow : SLOW;
    i : INT;
    n : DINT;
END_VAR
FOR i := 1 TO 5 DO
    slow();
END_FOR;
WHILE n < 3 DO
    n := QUICK(a := n);
END_WHILE;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// The result of building the project in `root` for this computer.
fn build_result(root: &Path) -> (Result<Build, String>, Toolchain) {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let result = build_project(
        root,
        &sources(root),
        &[],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .map_err(|error| error.to_string());
    (result, toolchain)
}

#[test]
fn a_task_whose_rust_work_may_outlast_its_interval_is_warned() {
    // RU-47: the task's Rust work is each budget times the most calls a scan makes: SLOW 5 × 4 ms
    // = 20 ms, more than the 10 ms interval; QUICK in a WHILE loop has no bound. Both are warnings
    // in the default `development` profile, and the image still builds.
    let root = custom_project(
        "rust-pous-admission",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TIMED)],
        TIMED_PROGRAM,
    );
    let (result, toolchain) = build_result(&root);
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        let error = result.err().expect("the build is refused");
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return;
    }
    let built = result.unwrap_or_else(|error| panic!("the project builds: {error}"));
    assert_eq!(
        built.warnings,
        [
            "the task Fast calls the Rust POU QUICK in a loop without a bound, so its Rust work per \
             scan has no bound (specification 48 RU-47)",
            "the task Fast may spend 20 ms per scan in Rust POUs (SLOW 5 × 4 ms), more than its \
             interval 10 ms (specification 48 RU-47)",
        ]
    );
}

#[test]
fn a_production_profile_makes_the_findings_errors() {
    // RU-47: `[timing] profile = "production"` refuses what `development` warns of, before the
    // crate compiles, whatever the toolchain.
    let root = custom_project(
        "rust-pous-production",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TIMED)],
        TIMED_PROGRAM,
    );
    std::fs::write(
        root.join("trust.toml"),
        "[timing]\nprofile = \"production\"\n",
    )
    .expect("trust.toml");
    measure_here(&root, &[("SLOW", 1), ("QUICK", 1)]); // RU-48: under the budgets
    let (result, _) = build_result(&root);
    let error = result.err().expect("the build is refused");
    assert!(
        error.contains("SLOW 5 × 4 ms") && error.contains("QUICK in a loop without a bound"),
        "{error}"
    );
}

#[test]
fn trust_toml_lowers_a_budget_and_never_raises_it() {
    // RU-46: `[timing.budgets]` may lower SLOW's budget for this target, here to 1 ms, so that
    // 5 × 1 ms fits the 10 ms interval; a budget above the POU's own 4 ms is refused.
    let root = custom_project(
        "rust-pous-budgets",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TIMED)],
        TIMED_PROGRAM,
    );
    let triple = trust_runtime::native::triple(Target::this_cpu().expect("a computer target"));
    let lowered = format!(
        "[timing]\nprofile = \"production\"\n[timing.budgets]\n\"SLOW\" = {{ \"{triple}\" = \"1ms\" }}\n"
    );
    std::fs::write(root.join("trust.toml"), &lowered).expect("trust.toml");
    measure_here(&root, &[("SLOW", 1), ("QUICK", 1)]); // RU-48: under the budgets
    let (result, _) = build_result(&root);
    let error = result.err().unwrap_or_default();
    // Only QUICK's loop is left, which production refuses before the crate compiles: SLOW's work
    // fits now.
    assert!(!error.contains("SLOW"), "{error}");
    assert!(error.contains("QUICK in a loop without a bound"), "{error}");
    std::fs::write(root.join("trust.toml"), lowered.replace("1ms", "10ms")).expect("trust.toml");
    let (result, _) = build_result(&root);
    let error = result.err().expect("the raised budget is refused");
    assert!(error.contains("never raise it"), "{error}");
}

/// Structured Text that reaches the Rust POUs of [`TIMED`] through a method in a FOR loop, a
/// property, an interface, an override behind a `VAR_IN_OUT`, and a function block instance that
/// the task runs on its own; a method that is never called holds QUICK in a WHILE loop.
const REACHED_PROGRAM: &str = "INTERFACE IWork
METHOD Run : DINT
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Worker IMPLEMENTS IWork
VAR
    slow : SLOW;
END_VAR
METHOD PUBLIC Run : DINT
slow();
Run := 0;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Counter
VAR
    n : DINT;
END_VAR
METHOD PUBLIC Advance : DINT
n := QUICK(a := n);
Advance := n;
END_METHOD
METHOD PUBLIC Unused : DINT
WHILE n < 10 DO
    n := QUICK(a := n);
END_WHILE;
Unused := n;
END_METHOD
PUBLIC PROPERTY Next : DINT
GET
    Next := QUICK(a := n);
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK Faster EXTENDS Counter
METHOD PUBLIC OVERRIDE Advance : DINT
n := QUICK(a := QUICK(a := n));
Advance := n;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION Twice : DINT
VAR_IN_OUT
    c : Counter;
END_VAR
Twice := c.Advance() + c.Advance();
END_FUNCTION

PROGRAM Line
VAR
    worker : Worker;
    work : IWork;
    counter : Counter;
    ticker : SLOW;
    i : INT;
    n : DINT;
END_VAR
work := worker;
FOR i := 1 TO 4 DO
    n := counter.Advance();
END_FOR;
n := counter.Next;
n := work.Run();
n := Twice(c := counter);
counter();
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#5ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line (ticker WITH Fast);
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_task_counts_the_rust_work_its_methods_properties_and_overrides_reach() {
    // RU-47: the calls are counted through the Structured Text that makes them. By hand, QUICK
    // (1 µs): `counter.Advance()` in the FOR loop, on a variable of Counter, runs Counter's
    // Advance, 4 × 1; the property read runs its GET, 1; `Twice` calls Advance twice on its
    // VAR_IN_OUT, which may hold a Faster, whose Advance calls QUICK twice, 2 × 2; 9 in all. SLOW
    // (4 ms): the interface call runs Worker's Run, the only implementation, 1; the task runs
    // `ticker` on its own, 1;
    // 2 in all. 9 µs + 8 ms = 8.009 ms, more than the 5 ms interval. `counter()` runs Counter's
    // own body, which has no statements: its never called method Unused, whose WHILE loop calls
    // QUICK without a bound, is not part of it. Catches a method's calls not counted or counted
    // once for a loop, a property, an interface or a task's block left out, a VAR_IN_OUT taken
    // for its declared type, a variable taken for a derived type, and a block's methods counted
    // as its body. The production profile refuses the program before the crate compiles.
    let root = custom_project(
        "rust-pous-reached",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TIMED)],
        REACHED_PROGRAM,
    );
    std::fs::write(
        root.join("trust.toml"),
        "[timing]\nprofile = \"production\"\n",
    )
    .expect("trust.toml");
    measure_here(&root, &[("SLOW", 1), ("QUICK", 1)]); // RU-48: under the budgets
    let (result, _) = build_result(&root);
    let error = result.err().expect("the build is refused");
    assert!(
        error.contains(
            "the task Fast may spend 8.009 ms per scan in Rust POUs (QUICK 9 × 1 µs, SLOW 2 × \
             4 ms), more than its interval 5 ms (specification 48 RU-47)"
        ),
        "{error}"
    );
    assert!(!error.contains("without a bound"), "{error}");
}

/// A function that counts its calls in a `static`, and one that recurses, neither of which can
/// panic.
const RULE_BREAKING: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicU32, Ordering};
use trust_plc::prelude::*;

static CALLS: AtomicU32 = AtomicU32::new(0);

#[trust_function(name = \"COUNTED\")]
#[budget(\"1us\")]
pub fn counted(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    CALLS.fetch_add(1, Ordering::Relaxed);
    Ok(a)
}

#[inline(never)]
fn fib(n: u32) -> u32 {
    if n < 2 {
        n
    } else {
        fib(n.saturating_sub(1)).wrapping_add(fib(n.saturating_sub(2)))
    }
}

#[trust_function(name = \"FIBONACCI\")]
#[budget(\"1us\")]
pub fn fibonacci(_cx: &mut Cx, n: u32) -> Result<u32, Fault> {
    Ok(fib(n))
}
";

#[test]
fn a_changing_static_and_a_recursion_through_rust_are_named() {
    // RU-13, RU-18, RU-45: the build refuses a module with a `static` that changes, which keeps
    // state outside the program's areas, and with a recursion through Rust, which the stack bound
    // and the call levels do not cover, naming the `static` and the function; for each computer's
    // CPU (specification 44 §4.4), whatever this computer's is. Catches a check that lets them
    // through, a refusal that names neither, and an AArch64 module whose atomics call the C
    // library's `getauxval` (the compiler's outline atomics), which failed its link before the
    // rules (macOS, 15 November 2026).
    let root = custom_project(
        "rust-pous-module-rules",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", RULE_BREAKING)],
        "PROGRAM Line\nVAR\n    a : DINT;\n    n : UDINT;\nEND_VAR\na := COUNTED(a := 1);\nn := FIBONACCI(n := 10);\nEND_PROGRAM\n",
    );
    let (error, toolchain) = build_error(&root);
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return;
    }
    for target in [Target::X86_64, Target::Aarch64] {
        let error = build_project(
            &root,
            &sources(&root),
            &[],
            &Options {
                target,
                hooks: true,
            },
            ConversionProfile::Iec,
            &toolchain,
            &root.join(format!(
                "build/native-{}",
                trust_runtime::native::triple(target)
            )),
        )
        .err()
        .expect("the build is refused")
        .to_string();
        assert!(
            error.contains("the `static` rules::CALLS changes")
                && error.contains("specification 48 RU-13"),
            "{target:?}: {error}"
        );
        assert!(
            error.contains("rules::fib calls itself") && error.contains("specification 48 RU-18"),
            "{target:?}: {error}"
        );
    }
}

/// A crate whose block gives the task's logical time at its call.
const CLOCKED: &str = r#"#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// The task's logical time and interval at its call.
#[trust_fb(name = "CLOCKED")]
#[budget("1us")]
pub struct Clocked {
    #[output] pub stamp: Time,
    #[output] pub interval: Time,
}

impl Body for Clocked {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.stamp = cx.now();
        self.interval = cx.interval();
        Ok(())
    }
}
"#;

#[test]
fn a_rust_pou_reads_the_logical_time_structured_text_reads() {
    // RU-16 and RU-11: a Rust POU reads the task's logical time from its own context, the time
    // Structured Text's `TIME()` reads in the same activation. Four calls at 0, 10, 20 and 30 ms (the
    // task is due at its start, specification 10 §4.3): both read 30 ms at the last. Catches a context
    // that loses or stales the activation's time.
    let root = custom_project(
        "rust-pous-logical-time",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", CLOCKED)],
        "PROGRAM Line\nVAR\n    clock : CLOCKED;\n    rust_time : TIME;\n    st_time : TIME;\nEND_VAR\n\
         clock();\nrust_time := clock.stamp;\nst_time := TIME();\nEND_PROGRAM\n\n\
         CONFIGURATION Probe\nRESOURCE Probe ON PLC\n    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);\n\
         \x20   PROGRAM Main WITH Fast : Line;\nEND_RESOURCE\nEND_CONFIGURATION\n",
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10, 20, 30] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    let thirty = Some(Value::Time(Duration::from_millis(30)));
    assert_eq!(program.value("MAIN.ST_TIME"), thirty);
    assert_eq!(program.value("MAIN.RUST_TIME"), thirty);
}

#[test]
fn a_rust_pou_reads_its_tasks_interval() {
    // RU-16 and specification 44 §8.2: `cx.interval()` is the running task's interval, T#10ms for
    // the instance Main in the task Fast; the instance Free runs in no task, at the resource's cycle
    // interval, which the resource gives the program, 20 ms here. Catches an interval that is not
    // the task's, and a program without a task that reads none.
    let root = custom_project(
        "rust-pous-interval",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", CLOCKED)],
        "PROGRAM Line\nVAR\n    clock : CLOCKED;\n    interval : TIME;\nEND_VAR\n\
         clock();\ninterval := clock.interval;\nEND_PROGRAM\n\n\
         CONFIGURATION Probe\nRESOURCE Probe ON PLC\n    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);\n\
         \x20   PROGRAM Main WITH Fast : Line;\n    PROGRAM Free : Line;\nEND_RESOURCE\n\
         END_CONFIGURATION\n",
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program.set_cycle_interval(Duration::from_millis(20));
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(
        program.value("MAIN.INTERVAL"),
        Some(Value::Time(Duration::from_millis(10)))
    );
    assert_eq!(
        program.value("FREE.INTERVAL"),
        Some(Value::Time(Duration::from_millis(20)))
    );
}

#[test]
fn a_crate_that_depends_on_the_sdks_interface_is_refused() {
    // RU-20: the interface crate's functions trust what the generated code hands them (`run` takes
    // the kernel's context), so the project's crate reaches the SDK through `trust-plc` only; one
    // that names `trust-plc-abi` itself is refused, naming both crates, before it compiles. Catches
    // a build that lets a project's code past the SDK's safe types.
    let version = env!("CARGO_PKG_VERSION");
    let root = custom_project(
        "rust-pous-sdk-interface",
        &[
            (
                "Cargo.toml",
                &manifest(&format!("trust-plc-abi = \"={version}\"\n")),
            ),
            (
                "src/lib.rs",
                &root("#![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n\n"),
            ),
        ],
        PASS_ON_PROGRAM,
    );
    let (error, toolchain) = build_error(&root);
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return;
    }
    let lines: Vec<&str> = error.lines().collect();
    assert_eq!(lines.len(), 1, "{error}");
    assert!(
        lines[0].starts_with(&format!(
            "rules 0.1.0 depends on trust-plc-abi {version}, which the SDK is built from"
        )) && lines[0].contains("RU-20"),
        "{error}"
    );
}
