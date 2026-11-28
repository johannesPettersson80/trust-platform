//! The single-axis demo `plcopen_motion_single_axis_demo` as its README writes it (specification 25
//! §3.5): the PLCopen motion blocks called every scan, the buffered move stopped at the software
//! limit, seven scans a sequence on the virtual axis, and the Try-its: a move that fits the limit,
//! the limit off, and a refused acceleration that takes the program to its fault step, which
//! removes the power. Each read is the README's `ctl read` of its globals.

use std::collections::HashMap;

use super::*;

/// The example's project under `examples`, and the library its `trust-lsp.toml` names by path.
const DEMO: &str = "plcopen_motion_single_axis_demo";
const LIBRARY: &str = "libraries/plcopen_motion/single_axis_core";

/// The globals that step 6 of the README reads, in its order.
const GLOBALS: [&str; 7] = [
    "g_motion_demo_current_step",
    "g_motion_demo_completed_sequences",
    "g_motion_demo_limit_clamp_verified",
    "g_motion_demo_last_position",
    "g_motion_demo_last_error",
    "g_motion_demo_power_on",
    "g_motion_demo_is_homed",
];

/// The most reads that wait for a step, which a running sequence passes once in seven scans.
const READS_FOR_A_STEP: usize = 400;

/// One read of the README's globals: the scan that left them and each global's printed line,
/// `<name> : <type> = <value>`.
struct Read {
    scan: u64,
    lines: HashMap<String, String>,
}

impl Read {
    fn line(&self, name: &str) -> &str {
        self.lines
            .get(name)
            .unwrap_or_else(|| panic!("the read has no {name}: {:?}", self.lines))
    }

    fn value(&self, name: &str) -> &str {
        let line = self.line(name);
        line.split_once(" = ")
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("no value in {line}"))
    }

    fn count(&self, name: &str) -> u64 {
        self.value(name)
            .parse()
            .unwrap_or_else(|_| panic!("{name} is not a count: {}", self.value(name)))
    }
}

/// `trust-runtime ctl read` of the README's globals, as step 6 runs it: a line with the scan, then
/// one `<name> : <type> = <value>` line for each global.
fn read_globals(runtime: &Runtime) -> Read {
    let mut args = vec!["read"];
    args.extend(GLOBALS);
    let text = runtime.ctl_ok(&args);
    let mut lines = text.lines();
    let scan = lines
        .next()
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|word| word.strip_prefix("scan="))
        })
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("the read starts with its scan: {text}"));
    let lines = lines
        .filter_map(|line| {
            let (name, _) = line.split_once(" : ")?;
            Some((name.to_string(), line.to_string()))
        })
        .collect();
    Read { scan, lines }
}

/// Reads until a read shows `step`.
fn read_in_step(runtime: &Runtime, step: &str) -> Read {
    for _ in 0..READS_FOR_A_STEP {
        let read = read_globals(runtime);
        if read.value("g_motion_demo_current_step") == step {
            return read;
        }
    }
    panic!(
        "no read of {READS_FOR_A_STEP} showed step {step}\n{}",
        runtime.diagnostics()
    );
}

/// The demo with `edits` of its `src/Main.st`, each a text and its replacement, running.
fn start(edits: &[(&'static str, &'static str)]) -> Runtime {
    let mut example = ExampleRuntime::new(DEMO).library(LIBRARY);
    for &(from, to) in edits {
        example = example.edit_source("src/Main.st", from, to);
    }
    let runtime = example.start();
    runtime.wait_until(WAIT, "the PLC to run", || {
        runtime.status().state == "running"
    });
    runtime
}

fn readme() -> String {
    std::fs::read_to_string(repo_root().join(format!("examples/{DEMO}/README.md")))
        .expect("the demo's README")
}

/// `text` with each run of white space as one space, so that a phrase matches however the README
/// wraps it.
fn prose(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_single_axis_demo_runs_its_sequence_as_its_readme_writes() {
    // examples/plcopen_motion_single_axis_demo/README.md, steps 5 and 6, worked from the program and
    // PLCopen Part 1: once the setup steps are done, the axis is powered and homed and no block
    // reports an error. The buffered relative move of +15 from 80.0 would end at 95, and the enabled
    // limit stops it at 90.0, which a read in step 50 shows with the check TRUE; a read in step 53
    // shows the axis at 80.0, between the two moves. On the virtual axis each step from 50 to 62
    // lasts one scan, so a sequence takes seven scans. Catches a sequence that stalls or errors, a
    // buffered move that the limit does not stop, a power lost on the healthy path, and a README
    // whose quoted values the program does not give.
    let readme = readme();
    let text = prose(&readme);
    let runtime = start(&[]);
    let status = runtime.ctl_ok(&["status"]);
    assert!(
        text.contains(&format!("It answers `{}`.", status.trim())),
        "the README quotes the status line {status}"
    );

    let in_step_50 = read_in_step(&runtime, "50");
    for (name, value) in [
        ("g_motion_demo_limit_clamp_verified", "true"),
        ("g_motion_demo_last_position", "90.0"),
        ("g_motion_demo_last_error", "0"),
        ("g_motion_demo_power_on", "true"),
        ("g_motion_demo_is_homed", "true"),
    ] {
        assert_eq!(in_step_50.value(name), value, "{name} in step 50");
        let line = in_step_50.line(name);
        assert!(
            readme.contains(&format!("    {line}\n")),
            "the README quotes `{line}`"
        );
    }
    let between = read_in_step(&runtime, "53");
    assert_eq!(between.value("g_motion_demo_last_position"), "80.0");
    assert!(text.contains("A read that shows step `53` finds the axis at `80.0`"));

    // Seven scans a sequence, measured over at least 100 scans: the counter and the scan of two
    // reads, each from one completed scan, differ by seven scans a sequence, give or take the one
    // sequence that either read may be in.
    let first = read_globals(&runtime);
    let mut second = read_globals(&runtime);
    while second.scan < first.scan + 100 {
        second = read_globals(&runtime);
    }
    let sequences = second.count("g_motion_demo_completed_sequences")
        - first.count("g_motion_demo_completed_sequences");
    let scans = second.scan - first.scan;
    assert!(
        scans.abs_diff(7 * sequences) <= 7,
        "{sequences} sequences in {scans} scans, not seven scans each"
    );
    assert!(text.contains("a sequence takes seven"));
}

#[test]
fn the_single_axis_demo_try_its_change_what_its_readme_says() {
    // examples/plcopen_motion_single_axis_demo/README.md, "Try it yourself", worked from the program
    // and PLCopen Part 1: a relative move of 5.0 ends at 80 + 5 = 85.0 inside the limit, so the check
    // is FALSE; with the limit off the move of 15.0 ends at 95.0; MC_MoveAbsolute refuses an
    // acceleration of -1.0 with mcERR_InvalidParameter (1), which the program latches, and its step
    // 900 removes the power (G348), leaving the axis homed at 0.0 with no sequence completed. A read
    // in step 60 comes after the relative move ended. Catches a limit that stops a move inside it or
    // stops one while it is off, an invalid acceleration that the block takes, and a fault step that
    // leaves the axis powered.
    let text = prose(&readme());

    let fits = start(&[("Distance := REAL#15.0,", "Distance := REAL#5.0,")]);
    let read = read_in_step(&fits, "60");
    assert_eq!(read.value("g_motion_demo_last_position"), "85.0");
    assert_eq!(read.value("g_motion_demo_limit_clamp_verified"), "false");
    assert!(text.contains(
        "`g_motion_demo_last_position` 85.0 and `g_motion_demo_limit_clamp_verified` FALSE"
    ));
    drop(fits);

    let open = start(&[(
        "    ParameterNumber := Constants.PN_EnableLimitPos,\n    Value := TRUE,",
        "    ParameterNumber := Constants.PN_EnableLimitPos,\n    Value := FALSE,",
    )]);
    let read = read_in_step(&open, "60");
    assert_eq!(read.value("g_motion_demo_last_position"), "95.0");
    assert_eq!(read.value("g_motion_demo_limit_clamp_verified"), "false");
    assert!(text.contains("showed the position 95.0 and the check FALSE"));
    drop(open);

    let refused = start(&[(
        "    Velocity := REAL#25.0,\n    Acceleration := REAL#0.0,",
        "    Velocity := REAL#25.0,\n    Acceleration := REAL#-1.0,",
    )]);
    let read = read_in_step(&refused, "900");
    for (name, value) in [
        ("g_motion_demo_last_error", "1"),
        ("g_motion_demo_power_on", "false"),
        ("g_motion_demo_is_homed", "true"),
        ("g_motion_demo_last_position", "0.0"),
        ("g_motion_demo_completed_sequences", "0"),
    ] {
        assert_eq!(read.value(name), value, "{name} in the fault step");
    }
    assert!(text.contains("`g_motion_demo_last_error` 1, `g_motion_demo_power_on` FALSE"));
}
