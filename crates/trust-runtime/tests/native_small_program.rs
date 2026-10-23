//! A program without read-only data compiles into a module whose code starts at address 0, for the
//! C6 and for this computer, and runs (specification 44 §4.5, §5.3); the stack bound holds the
//! frames of the calls a program makes (§8.5).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{
    build, build_with_tests, load, Device, NoHooks, Options, Target, Toolchain,
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
