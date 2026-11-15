//! A project with Rust POUs builds into one program image with its crate, and its Structured Text
//! calls them as it calls its own POUs (specification 48 §6.3, §9): a Rust function block runs its
//! body on its instance's variables, a Rust function returns its result, and a fault a Rust POU
//! returns ends the scan with its code, leaving the instance as it was (RU-14, RU-28).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

use std::path::{Path, PathBuf};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::rust_pous::{write_declarations, Written, DECLARATIONS_FILE};
use trust_runtime::native::{
    build_project, load, Build, Device, NoHooks, Options, Target, Toolchain,
};
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

/// The project's Structured Text, as `trust-runtime build` collects it.
fn sources(root: &Path) -> Vec<SourceFile> {
    ["src/main.st", DECLARATIONS_FILE]
        .into_iter()
        .map(|label| {
            let text = std::fs::read_to_string(root.join(label)).expect("the source");
            SourceFile::with_path(label, text)
        })
        .collect()
}

/// The project built for this computer; `None` with a toolchain that has no cargo and SDK sources,
/// whose refusal names them.
fn built(root: &Path) -> Option<(Build, Device)> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    let work = root.join("build/native");
    let result = build_project(
        root,
        &sources(root),
        &[],
        &Options {
            target,
            hooks: true,
        },
        ConversionProfile::Iec,
        &toolchain,
        &work,
    );
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        let error = result.err().expect("the build is refused").to_string();
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return None;
    }
    let built = result.unwrap_or_else(|error| panic!("the project builds: {error}"));
    let device = Device {
        target,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
        trusted_keys: &[],
    };
    Some((built, device))
}

#[test]
fn structured_text_calls_rust_pous_as_its_own() {
    // Catches a build that leaves the Rust POUs out or calls them with their variables in other
    // places: three calls at 10, 20 and 30 ms (the task is not due at 0, specification 10 §4.3).
    // The level is 2, 4, 6; the filter primes with 2, then moves by half the difference: 2 +
    // 0.5 × (4 − 2) = 3, then 3 + 0.5 × (6 − 3) = 4.5. MAX_LIMITED(3, 7, 5) is min(max(3, 7), 5) = 5.
    // The counter's level 200 is above 100 at each call: high, and a count of 3.
    let root = project("rust-pous-line", LINE);
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

/// A project with the crate files `files` (relative to `rust/`) and the program `program`.
fn custom_project(name: &str, files: &[(&str, &str)], program: &str) -> PathBuf {
    let root = trust_test_support::unique_temp_path(name);
    for (path, text) in files {
        let file = root.join("rust").join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    std::fs::create_dir_all(root.join("src")).expect("the sources' folder");
    std::fs::write(root.join("src/main.st"), program).expect("the program");
    let written = write_declarations(&root);
    assert!(matches!(written, Ok(Written::Wrote(_))), "{written:?}");
    root
}

/// The crate's manifest with the dependencies `extra`.
fn manifest(extra: &str) -> String {
    format!(
        "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
         [dependencies]\ntrust-plc = \"={}\"\n{extra}",
        env!("CARGO_PKG_VERSION")
    )
}

/// A crate root with the function `PASS_ON`, after the attributes `head`.
fn root(head: &str) -> String {
    format!(
        "{head}use trust_plc::prelude::*;\n\n#[trust_function(name = \"PASS_ON\")]\n#[budget(\"1us\")]\n\
         pub fn pass_on(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {{\n    Ok(a)\n}}\n"
    )
}

/// The error of building the project in `root` for this computer.
fn build_error(root: &Path) -> (String, Toolchain) {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let error = build_project(
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
    .err()
    .expect("the build is refused")
    .to_string();
    (error, toolchain)
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

/// A block that computes with a duration, a bit string and a character.
const TYPED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"TYPED_BLOCK\")]
#[budget(\"1us\")]
pub struct TypedBlock {
    #[input]
    pub period: Time,
    #[input]
    pub flags: Byte,
    #[input]
    pub letter: Char,
    #[output]
    pub half: Time,
    #[output]
    pub low: Byte,
    #[output]
    pub next: Char,
    #[output]
    #[init(\"T#1s\")]
    pub total: Time,
}

impl Body for TypedBlock {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.half = Time::from_nanos(self.period.as_nanos() / 2);
        self.low = self.flags & Byte(0x0F);
        self.next = Char(self.letter.0 + 1);
        self.total = self.total.checked_add(cx, self.period)?;
        Ok(())
    }
}
";

/// A program that calls TYPED_BLOCK once per 10 ms with IEC literals.
const TYPED_PROGRAM: &str = "PROGRAM Line
VAR
    block : TYPED_BLOCK;
    half : TIME;
    low : BYTE;
    next : CHAR;
    total : TIME;
END_VAR
block(period := T#500ms, flags := 16#AB, letter := 'Y');
half := block.half;
low := block.low;
next := block.next;
total := block.total;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_rust_pou_computes_with_durations_bit_strings_and_characters() {
    // Catches a TIME, BYTE or CHAR that reaches the Rust POU as another value, or comes back so:
    // RU-25's `Time`, `Byte` and `Char` hold what the areas store (specification 44 §6.2). Three
    // calls at 10, 20 and 30 ms: half of T#500ms is T#250ms, 16#AB AND 16#0F is 16#0B, 'Y' + 1 is
    // 'Z', and the total starts at its declared T#1s (RU-26) and grows by T#500ms at each call:
    // T#2.5s.
    let root = custom_project(
        "rust-pous-typed",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TYPED)],
        TYPED_PROGRAM,
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
    assert_eq!(
        program.value("MAIN.HALF"),
        Some(Value::Time(Duration::from_millis(250)))
    );
    assert_eq!(program.value("MAIN.LOW"), Some(Value::Byte(0x0B)));
    assert_eq!(program.value("MAIN.NEXT"), Some(Value::Char(b'Z')));
    assert_eq!(
        program.value("MAIN.TOTAL"),
        Some(Value::Time(Duration::from_millis(2_500)))
    );
}
