//! A program without read-only data compiles into a module whose code starts at address 0, for the
//! C6 and for this computer, and runs (specification 44 §4.5, §5.3); the stack bound holds the
//! frames of the calls a program makes (§8.5).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{
    build, build_with_tests, load, Build, Device, NoHooks, Options, Target, Toolchain,
};
use trust_runtime::value::{Duration, Value};

/// A counter in a 10 ms task: no constant table, no text, nothing a module keeps read-only.
const COUNTER: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
END_VAR
Count := Count + 1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_program_without_read_only_data_builds_for_the_c6_and_runs_here() {
    // A module whose linker placed its dynamic symbol tables before its code was refused with "the
    // module's code does not start at address 0" for the C6 (22 October 2026, the fault probe); the
    // reference workload has read-only data and did not show it. Catches the tables placed before
    // the code again, for either target.
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let sources = [SourceFile::new(COUNTER)];
    let host = Target::this_cpu().expect("a computer target");
    for target in [Target::Rv32imac, host] {
        let work = std::env::temp_dir().join(format!(
            "trust-native-small-{}-{target:?}",
            std::process::id()
        ));
        let built = build(
            &sources,
            &Options {
                target,
                hooks: true,
            },
            &toolchain,
            &work,
        )
        .unwrap_or_else(|error| panic!("the counter builds for {target:?}: {error}"));
        let _ = std::fs::remove_dir_all(&work);
        assert_eq!(built.figure.rodata, 0, "the counter has no read-only data");
        if target != host {
            continue;
        }
        let device = Device {
            target,
            program_memory: 1 << 20,
            task_stack: 1 << 20,
            max_statements: 1_000,
        };
        let mut program = load(&built.image, &device).expect("the image loads");
        program.cold_start(Duration::ZERO).expect("the cold start");
        // The task is due at 10, 20 and 30 ms, not at its start (specification 10 §4.3).
        for time in [0, 10, 20, 30] {
            program
                .scan(Duration::from_millis(time), &mut NoHooks)
                .expect("the scan runs");
        }
        assert_eq!(program.value("MAIN.COUNT"), Some(Value::UDInt(3)));
    }
}

/// A loop that never ends, in a 10 ms task: each pass adds one to `Turns`.
const ENDLESS: &str = "
PROGRAM Spinner
VAR
    Turns : UDINT;
END_VAR
WHILE TRUE DO
    Turns := Turns + 1;
END_WHILE;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Spinner;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_release_image_counts_only_loop_passes_and_still_ends_an_endless_loop() {
    // Specification 44 §13.2 and NX-10: an image built without statement hooks is a release image,
    // which bit 0 of its header flags says, and its budget counts only the passes through loops.
    // With a budget of 1,000, the release image's scan runs 1,000 passes of the loop, each adding
    // one to `Turns`, and the 1,001st pass ends it with `InfiniteLoop`. The debug image of the same
    // program counts the WHILE statement (999 left), then each pass and its assignment (two each):
    // 499 passes leave one, the 500th pass takes it, and its assignment ends the scan before it
    // stores, so `Turns` is 499. The stores before the fault stay (specification 11 §6.6). Catches
    // a release image that loses the loop's count (its scan would never end), one that still
    // carries the statement hooks (499), a debug image without them (1,000), and a header that
    // does not tell the two apart.
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    for (hooks, release, turns) in [(false, true, 1_000), (true, false, 499)] {
        let work = std::env::temp_dir().join(format!(
            "trust-native-release-{}-{hooks}",
            std::process::id()
        ));
        let built = build(
            &[SourceFile::new(ENDLESS)],
            &Options { target, hooks },
            &toolchain,
            &work,
        )
        .unwrap_or_else(|error| panic!("the spinner builds (hooks {hooks}): {error}"));
        let _ = std::fs::remove_dir_all(&work);
        let device = Device {
            target,
            program_memory: 1 << 20,
            task_stack: 1 << 20,
            max_statements: 1_000,
        };
        let mut program = load(&built.image, &device).expect("the image loads");
        assert_eq!(
            program.image().header().is_release(),
            release,
            "hooks {hooks}"
        );
        program.cold_start(Duration::ZERO).expect("the cold start");
        // The task is first due at 10 ms (specification 10 §4.3).
        program
            .scan(Duration::ZERO, &mut NoHooks)
            .expect("no task is due at the start");
        let error = program
            .scan(Duration::from_millis(10), &mut NoHooks)
            .expect_err("the endless loop ends the scan");
        assert!(
            matches!(
                error,
                RuntimeError::ExecutionLimit {
                    kind: ExecutionLimitKind::Work,
                    ..
                }
            ),
            "hooks {hooks}: {error:?}"
        );
        assert_eq!(
            program.value("MAIN.TURNS"),
            Some(Value::UDInt(turns)),
            "hooks {hooks}"
        );
    }
}

/// An array read and written at constant indexes and at a computed one.
const INDEXES: &str = "
PROGRAM Indexes
VAR
    Table : ARRAY[1..4] OF INT := [10, 20, 30, 40];
    i : INT := 3;
    First : INT;
    Last : INT;
    Computed : INT;
END_VAR
First := Table[1];
Last := Table[4];
Table[2] := 25;
Computed := Table[i] + Table[2];
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Indexes;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_constant_index_inside_the_bounds_is_not_checked() {
    // Specification 44 §6.5: the array index is checked wherever it is not a constant inside the
    // bounds. `Table[1]`, `Table[4]` and `Table[2]` are constants inside 1..4, so the module indexes
    // the array directly, and only the computed `Table[i]` goes through the library's check, the
    // one `lib::at` of the generated source. After one scan `First` is 10, `Last` 40, `Table[2]`
    // 25 and `Computed` 30 + 25 = 55. Catches a constant index checked when the scan runs, and a
    // constant index that misses the lower bound (`First` would read 20, or the build fail).
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-indexes-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(INDEXES)],
        &Options {
            target,
            hooks: false,
        },
        &toolchain,
        &work,
    )
    .unwrap_or_else(|error| panic!("the program builds: {error}"));
    let _ = std::fs::remove_dir_all(&work);
    assert_eq!(
        built.source.matches("lib::at(").count(),
        1,
        "{}",
        built.source
    );
    let device = Device {
        target,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("no task is due at the start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    for (variable, expected) in [("MAIN.FIRST", 10), ("MAIN.LAST", 40), ("MAIN.COMPUTED", 55)] {
        assert_eq!(
            program.value(variable),
            Some(Value::Int(expected)),
            "{variable}"
        );
    }
}

/// A project without a CONFIGURATION, so its program runs in the background, and a test program;
/// each counts how often its body ran.
const WITH_TEST: &str = "
PROGRAM Plant
VAR
    Scans : UDINT;
END_VAR
Scans := Scans + 1;
END_PROGRAM

TEST_PROGRAM CountsItsRuns
VAR
    Runs : UDINT;
END_VAR
Runs := Runs + 1;
END_TEST_PROGRAM
";

#[test]
fn a_test_image_runs_its_test_programs_only_when_invoked() {
    // Specification 22: a test program runs only when its test is invoked, and on a device the image
    // runs no test program by itself, so a start of the device, at a reset for example, runs only the
    // project's own programs (specification 44 §5.6). Two scans run the background program `Plant`
    // twice and the test program never; then one invocation runs the test program's body once.
    // Catches a test program listed in the background task, which the C6 ran at every boot: test
    // images whose assertions failed then faulted at once, and a faulted chip takes no install
    // (bench, 23 October 2026).
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-tests-{}", std::process::id()));
    let built = build_with_tests(
        &[SourceFile::new(WITH_TEST)],
        &["CountsItsRuns".into()],
        &Options {
            target,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .unwrap_or_else(|error| panic!("the project with its test builds: {error}"));
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("PLANT.SCANS"), Some(Value::UDInt(2)));
    assert_eq!(program.value("COUNTSITSRUNS.RUNS"), Some(Value::UDInt(0)));
    let test = program
        .image()
        .find_program_instance("CountsItsRuns")
        .expect("the test program is an instance");
    program
        .run_instance(test, Duration::from_millis(20))
        .expect("the test program runs");
    assert_eq!(program.value("COUNTSITSRUNS.RUNS"), Some(Value::UDInt(1)));
    assert_eq!(program.value("PLANT.SCANS"), Some(Value::UDInt(2)));
}

/// A function whose frame holds a 1,000-element `DINT` array, 4,000 bytes, which the program's body
/// calls in every scan; the array's cells are read and written at indexes the compiler cannot know.
const DEEP: &str = "
FUNCTION Deep : DINT
VAR_INPUT
    seed : DINT;
END_VAR
VAR
    cells : ARRAY[0..999] OF DINT;
    i : INT;
END_VAR
FOR i := 0 TO 999 DO
    cells[(i * 7 + seed) MOD 1000] := cells[i] + i;
END_FOR;
Deep := cells[seed MOD 1000];
END_FUNCTION

PROGRAM Caller
VAR
    Seed : DINT := 7;
    Result : DINT;
END_VAR
Seed := Seed + 1;
Result := Deep(seed := Seed);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Caller;
END_RESOURCE
END_CONFIGURATION
";

/// The stack bound of the C6's image holds the frame of the function the body calls (specification
/// 44 §8.5): `Deep`'s 4,000-byte array lies on the chain from the task's entry, inlined into the
/// body or called, so the bound is at least (4,000 + 4,096) × 1.25 = 10,120 bytes, and it fits the
/// C6's task stack of 43,816 bytes. Catches a bound that loses a callee's frame, which a call the
/// call graph does not resolve would drop; and, as the program's cold-start and warm-start
/// initializations hold the same code, which the link folds into one function under both names, a
/// bound that finds an entry point only under the name the folding kept (the build refused every
/// such program with "the module has no entry point trust_entry_warm").
#[test]
fn the_stack_bound_holds_the_frame_of_a_called_function() {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!("trust-native-deep-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(DEEP)],
        &Options {
            target: Target::Rv32imac,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the caller builds for the C6");
    let _ = std::fs::remove_dir_all(&work);
    let stack = built.figure.stack;
    assert!(
        (10_120..=43_816).contains(&stack),
        "stack bound {stack} bytes"
    );
}

/// The `runtime.toml` of a project whose resource runs a cycle every 10 ms, with the sections every
/// project must have at their defaults.
const RUNTIME_TOML: &str = "[bundle]
version = 1

[resource]
name = \"Probe\"
cycle_interval_ms = 10

[runtime.control]
endpoint = \"unix:///tmp/trust-native-bench.sock\"
mode = \"production\"
debug_enabled = false

[runtime.log]
level = \"info\"

[runtime.retain]
mode = \"none\"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = \"halt\"

[runtime.fault]
policy = \"halt\"
";

/// The report of `trust-runtime bench project --execution-backend native` on the counter project in
/// folder `name`, with `settings` as its `trust-lsp.toml` when given: one warmup and four measured
/// cycles, watching `Main.Count`.
fn bench_counter(name: &str, settings: Option<&str>) -> serde_json::Value {
    let project = std::env::temp_dir().join(format!("trust-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).expect("create the project");
    std::fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    std::fs::write(project.join("src/main.st"), COUNTER).expect("write the program");
    if let Some(settings) = settings {
        std::fs::write(project.join("trust-lsp.toml"), settings).expect("write the settings");
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["bench", "project", "--project"])
        .arg(&project)
        .args([
            "--samples",
            "4",
            "--warmup-cycles",
            "1",
            "--watch",
            "Main.Count",
            "--execution-backend",
            "native",
            "--output",
            "json",
        ])
        .output()
        .expect("run trust-runtime bench");
    let _ = std::fs::remove_dir_all(&project);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the bench writes JSON");
    report["report"].clone()
}

#[test]
fn bench_project_runs_the_project_compiled_on_the_kernel() {
    // Specification 44 §14.3: `bench project --execution-backend native` runs the project compiled
    // for this computer, each cycle one scan of the kernel at the next 10 ms of logical time. One
    // warmup and four measured cycles at 10 to 50 ms each run the task `Fast`, due first 10 ms after
    // the start (specification 10 §4.3), so `Main.Count` is 5, and the task's four measured
    // activations are timed. The report names the native backend and the image it built, a debug
    // image for a project without settings (§13.2). Catches a bench that runs the bytecode engine
    // under the native name, a cycle that does not advance the logical time (the task would run
    // once), warmups timed as measured cycles, and a release image by default.
    let report = bench_counter("bench", None);
    assert_eq!(report["execution_backend"], "native");
    assert_eq!(report["watched_globals"]["Main.Count"], 5);
    assert_eq!(report["tasks"][0]["name"], "Fast");
    assert_eq!(report["tasks"][0]["activations"], 4);
    assert_eq!(report["native_image"]["debug"], true, "{report}");
    assert!(
        report["native_image"]["code"].as_u64().unwrap_or(0) > 0,
        "the report names the image's code: {report}"
    );
}

#[test]
fn bench_project_measures_a_release_image_when_the_project_asks_for_one() {
    // Specification 44 §13.2 and §14.3: a project whose `trust-lsp.toml` says `[build] debug = false`
    // is measured as a release image, which the report says, and runs as the debug image does:
    // `Main.Count` is 5 after the same cycles. Its code is smaller than the debug image's, which
    // carries a hook for each statement. Catches a bench that ignores the setting, the measurement
    // M4 compares with RuSTy.
    let release = bench_counter("bench-release", Some("[build]\ndebug = false\n"));
    assert_eq!(release["native_image"]["debug"], false, "{release}");
    assert_eq!(release["watched_globals"]["Main.Count"], 5);
    assert_eq!(release["tasks"][0]["activations"], 4);
    let debug = bench_counter("bench-debug", Some("[build]\ndebug = true\n"));
    assert_eq!(debug["native_image"]["debug"], true, "{debug}");
    let code = |report: &serde_json::Value| report["native_image"]["code"].as_u64().unwrap_or(0);
    assert!(
        code(&release) < code(&debug),
        "release code {} bytes, debug code {} bytes",
        code(&release),
        code(&debug)
    );
}

/// `source` built for this computer, a debug image with `hooks` and a release image without
/// (specification 44 §13.2), and the device it runs on.
fn built_here(source: &str, label: &str, hooks: bool) -> (Build, Device) {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!(
        "trust-native-{label}-{hooks}-{}",
        std::process::id()
    ));
    let built = build(
        &[SourceFile::new(source)],
        &Options { target, hooks },
        &toolchain,
        &work,
    )
    .unwrap_or_else(|error| panic!("{label} builds: {error}"));
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
    };
    (built, device)
}

/// A chain of real `+ - *` on variables.
const CHAIN: &str = "
PROGRAM Chain
VAR
    A : REAL := 1.5;
    B : REAL := 2.0;
    C : REAL := 0.25;
    D : REAL := 1.0;
    Result : REAL;
END_VAR
Result := A * B + C - D;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Chain;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_chain_of_real_operations_is_checked_at_its_last_operation() {
    // Specification 44 §6.5: in a chain of real `+ - *` whose operands are variables and literals,
    // only the last operation checks that its result is finite. `A * B + C - D` is
    // `((A * B) + C) - D`, so the module checks the subtraction alone: one `lib::fsub` and no
    // `lib::fmul` or `lib::fadd`. The result is 1.5 * 2.0 + 0.25 - 1.0 = 2.25, exact in binary32.
    // Catches every operation of the chain checked, and an operation of the chain computed wrong
    // when it is not checked.
    let (built, device) = built_here(CHAIN, "chain", false);
    for (function, count) in [("lib::fsub(", 1), ("lib::fadd(", 0), ("lib::fmul(", 0)] {
        assert_eq!(
            built.source.matches(function).count(),
            count,
            "{function}\n{}",
            built.source
        );
    }
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.RESULT"), Some(Value::Real(2.25)));
}

/// `Big * 2.0` leaves the range of `REAL`; the chain multiplies the infinity by zero.
const OVERFLOW_IN_A_CHAIN: &str = "
PROGRAM Overflowing
VAR
    Big : REAL := 3.0E38;
    Result : REAL := 7.0;
END_VAR
Result := Big * 2.0 * 0.0;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Overflowing;
END_RESOURCE
END_CONFIGURATION
";

/// `Big * 2.0` leaves the range of `REAL` before a call that counts itself.
const OVERFLOW_BEFORE_A_CALL: &str = "
VAR_GLOBAL
    Calls : DINT;
END_VAR

FUNCTION Counted : REAL
VAR_INPUT
    x : REAL;
END_VAR
VAR_EXTERNAL
    Calls : DINT;
END_VAR
Calls := Calls + 1;
Counted := x;
END_FUNCTION

PROGRAM Overflowing
VAR
    Big : REAL := 3.0E38;
    Result : REAL := 7.0;
END_VAR
Result := Big * 2.0 + Counted(x := 1.0);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Overflowing;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_real_overflow_in_a_chain_ends_the_statement_where_its_operation_would() {
    // Specification 05 §7.1: a result outside its type is an overflow, the operands are evaluated
    // left first, and the error aborts the assignment before its store. Specification 44 §6.5
    // checks a chain of real `+ - *` at its last operation only where nothing between can fault
    // or write. 3.0E38 * 2.0 is beyond the largest `REAL`, 3.4028235E38, so both statements end
    // the scan with `Overflow` and `Result` keeps 7.0. In the first, the module's only check is
    // the last multiplication's, and the infinity times zero is NaN, which is not finite either.
    // In the second, a call follows the overflow, so the multiplication is checked itself and
    // `Counted` never runs: `Calls` stays 0. Catches a chain checked for infinity only (NaN would
    // be stored), and a chain checked at its end across a call (`Calls` would be 1).
    let (built, device) = built_here(OVERFLOW_IN_A_CHAIN, "overflow-chain", false);
    assert_eq!(
        built.source.matches("lib::fmul(").count(),
        1,
        "{}",
        built.source
    );
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the overflow ends the scan");
    assert_eq!(error, RuntimeError::Overflow);
    assert_eq!(program.value("MAIN.RESULT"), Some(Value::Real(7.0)));

    let (built, device) = built_here(OVERFLOW_BEFORE_A_CALL, "overflow-call", false);
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the overflow ends the scan");
    assert_eq!(error, RuntimeError::Overflow);
    assert_eq!(program.value("CALLS"), Some(Value::DInt(0)));
    assert_eq!(program.value("MAIN.RESULT"), Some(Value::Real(7.0)));
}

/// Three loops over an array: to the end, to an `EXIT`, and past the bounds.
const LOOPS: &str = "
PROGRAM Loops
VAR
    Values : ARRAY[1..4] OF INT := [10, 20, 30, 40];
    I : INT;
    J : INT;
    K : INT;
    Total : INT;
END_VAR
FOR I := 1 TO 4 DO
    Total := Total + Values[I];
END_FOR;
FOR J := 1 TO 4 DO
    IF Values[J] = 30 THEN
        EXIT;
    END_IF;
END_FOR;
FOR K := 4 TO 10 DO
    Total := Total + Values[K];
END_FOR;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Loops;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_release_image_holds_a_loops_control_variable_and_writes_it_at_each_pass() {
    // Specification 44 §6.6: a release image holds a `FOR` loop's control variable in a local when
    // nothing in the body but the loop can write it, and writes the variable at the start of each
    // pass; the debug image reads it where it lies. The first loop adds 10 + 20 + 30 + 40 = 100
    // and leaves `I` at 5, the value that ended it (IEC §7.3.3.4.2); the second leaves `J` at 3,
    // where `EXIT` left it; the third adds `Values[4]`, 140, and ends the scan at `K` = 5 with the
    // index fault for 5 outside 1..4, `K` holding 5. Both images give these values; the release
    // image's body reads no control variable from its place. Catches a held variable not written
    // back at the end, at an `EXIT` or before a fault, and a release image that reads it anyway.
    for hooks in [true, false] {
        let (built, device) = built_here(LOOPS, "loops", hooks);
        for variable in ["me.I.get()", "me.J.get()", "me.K.get()"] {
            let reads = built.source.matches(variable).count();
            assert_eq!(
                reads > 0,
                hooks,
                "hooks {hooks}: {variable}\n{}",
                built.source
            );
        }
        let mut program = load(&built.image, &device).expect("the image loads");
        program.cold_start(Duration::ZERO).expect("the cold start");
        let error = program
            .scan(Duration::from_millis(10), &mut NoHooks)
            .expect_err("the third loop indexes past the bounds");
        assert_eq!(
            error,
            RuntimeError::IndexOutOfBounds {
                index: 5,
                lower: 1,
                upper: 4
            },
            "hooks {hooks}"
        );
        for (variable, expected) in [
            ("MAIN.I", 5),
            ("MAIN.J", 3),
            ("MAIN.K", 5),
            ("MAIN.TOTAL", 140),
        ] {
            assert_eq!(
                program.value(variable),
                Some(Value::Int(expected)),
                "hooks {hooks}: {variable}"
            );
        }
    }
}

/// Two loops whose control variable changes in their body other than by an assignment there: a
/// method of the function block adds 5 to it, and a global that a `VAR_IN_OUT` names adds 4.
const WRITERS: &str = "
VAR_GLOBAL
    G : INT;
END_VAR

FUNCTION_BLOCK Stepper
VAR_OUTPUT
    I : INT;
    Passes : INT;
END_VAR
METHOD PUBLIC Skip
I := I + 5;
END_METHOD
FOR I := 1 TO 10 DO
    Passes := Passes + 1;
    THIS.Skip();
END_FOR;
END_FUNCTION_BLOCK

FUNCTION Run : INT
VAR_IN_OUT
    X : INT;
END_VAR
VAR_EXTERNAL
    G : INT;
END_VAR
VAR
    N : INT;
END_VAR
FOR X := 1 TO 10 DO
    N := N + 1;
    G := G + 4;
END_FOR;
Run := N;
END_FUNCTION

PROGRAM Writers
VAR
    S : Stepper;
    Stepped : INT;
    Last : INT;
    Aliased : INT;
END_VAR
S();
Stepped := S.Passes;
Last := S.I;
Aliased := Run(X := G);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Writers;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_loop_whose_body_can_reach_its_control_variable_reads_it_at_each_use() {
    // Specification 44 §6.6: a release image holds no control variable that a call or a
    // `VAR_IN_OUT` in the body can reach. IEC §7.3.3.4.2 tests the variable at each pass. In
    // `Stepper` the pass at I = 1 runs `Skip`, I = 6, the increment makes 7, the pass at 7 makes
    // 12 and the increment 13, past 10: two passes, `I` 13. In `Run`, `X` is `G`: the pass at 1
    // adds 4, G = 5, the increment makes 6, the pass at 6 makes 10 and the increment 11: two
    // passes, `G` 11. Catches a held variable that misses a write by a method or through an
    // alias: ten passes each.
    let (built, device) = built_here(WRITERS, "writers", false);
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    for (variable, expected) in [
        ("MAIN.STEPPED", 2),
        ("MAIN.LAST", 13),
        ("MAIN.ALIASED", 2),
        ("G", 11),
    ] {
        assert_eq!(
            program.value(variable),
            Some(Value::Int(expected)),
            "{variable}"
        );
    }
}
