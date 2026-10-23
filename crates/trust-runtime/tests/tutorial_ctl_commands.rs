//! The documented `ctl` sequences, run as written against the example each document names. Only
//! the project path and the generic `<project-folder>` placeholder are substituted. The runtime is
//! built and started as the tutorials start it, from a private copy of the example.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use runtime_process::{
    repo_root, stderr_text, trust_runtime_command, ExampleRuntime, Runtime, WAIT,
};

#[test]
fn memory_marker_counter_readme_steps_one_to_three_hold() {
    // README step 3: `trust-runtime ctl --project examples/memory_marker_counter io-write %MW0 41`,
    // then one cycle later `%QW0 = Int(41)` (the latched cycle-start value) and `%MW0 = Int(42)`
    // (the write-back at cycle end). The printed form is the one io-read gives INT bindings. Then
    // `%MW0` increments by 1 per cycle.
    let runtime = ExampleRuntime::new("memory_marker_counter").start();
    let write = runtime.prove_counter_write(41, 1);
    let first = &write.after;

    // The write landed on cycle `landing_scan`: %QW0 latched 41 there and %MW0 wrote 42 back. The
    // counter then moves one per cycle, so this snapshot's cycle is that many cycles later.
    assert_eq!(
        first.int("%QW0") + 1,
        first.int("%MW0"),
        "%QW0 must sit one below the write-back"
    );
    let cycles_since_write = first.scan() as i64 - write.landing_scan;
    assert_eq!(
        first.int("%QW0"),
        41 + cycles_since_write,
        "%QW0 must latch 41 at the landing cycle, then one per cycle"
    );
    assert_eq!(
        first.int("%MW0"),
        42 + cycles_since_write,
        "%MW0 must write 42 back at the landing cycle, then one per cycle"
    );
    assert_eq!(first.text("%QW0"), format!("Int({})", first.int("%QW0")));
    assert_eq!(first.text("%MW0"), format!("Int({})", first.int("%MW0")));

    // %MW0 increments by 1 per cycle: a later snapshot differs by exactly the cycles between.
    let later =
        runtime.wait_for_snapshot(WAIT, "two more cycles", |s| s.scan() >= first.scan() + 2);
    let cycles = later.scan() as i64 - first.scan() as i64;
    assert_eq!(
        later.int("%MW0") - first.int("%MW0"),
        cycles,
        "%MW0 must increment by one per cycle"
    );
}

#[test]
fn memory_marker_counter_readme_step_five_adds_five_per_cycle() {
    // README step 5 changes `Counter := Counter + 1;` to `+ 5;` in the copy, then repeats step 3.
    // %MW0 must climb by 5 per cycle, and the same landing arithmetic applies with a step of 5.
    let runtime = ExampleRuntime::new("memory_marker_counter")
        .edit_source(
            "src/Main.st",
            "Counter := Counter + 1;",
            "Counter := Counter + 5;",
        )
        .start();
    let write = runtime.prove_counter_write(41, 5);
    let first = &write.after;

    assert_eq!(
        first.int("%QW0") + 5,
        first.int("%MW0"),
        "%QW0 must sit five below the write-back"
    );
    let cycles_since_write = first.scan() as i64 - write.landing_scan;
    assert_eq!(
        first.int("%QW0"),
        41 + 5 * cycles_since_write,
        "%QW0 must latch 41 at the landing cycle, then five per cycle"
    );
    assert_eq!(
        first.int("%MW0"),
        46 + 5 * cycles_since_write,
        "%MW0 must write 46 back at the landing cycle, then five per cycle"
    );
    assert_eq!(first.text("%QW0"), format!("Int({})", first.int("%QW0")));
    assert_eq!(first.text("%MW0"), format!("Int({})", first.int("%MW0")));

    let later =
        runtime.wait_for_snapshot(WAIT, "two more cycles", |s| s.scan() >= first.scan() + 2);
    let cycles = later.scan() as i64 - first.scan() as i64;
    assert_eq!(
        later.int("%MW0") - first.int("%MW0"),
        5 * cycles,
        "%MW0 must jump by 5 per cycle"
    );
}

#[test]
fn communication_readme_io_read_reads_the_process_image() {
    // examples/communication/README.md ends its validation loop with `ctl --project . io-read`.
    // The folder holds one project per protocol. This runs the OPC UA project, whose driver is
    // simulated. The others need an EtherCAT, GPIO, Modbus or MQTT peer, or an ADS route.
    let runtime = ExampleRuntime::new("communication/opcua").start();
    runtime.wait_for_snapshot(WAIT, "the first I/O snapshot", |_| true);

    let result = runtime.ctl_result(&["io-read"]);
    assert!(
        result["snapshot"]["scan"].is_u64(),
        "io-read must return a scan number: {result}"
    );
    for area in ["inputs", "outputs", "memory"] {
        assert!(
            result["snapshot"][area].is_array(),
            "io-read snapshot has no `{area}` array: {result}"
        );
    }
}

#[test]
fn plcopen_motion_demo_readme_status_inspects_the_runtime() {
    // examples/plcopen_motion_single_axis_demo/README.md: `ctl --project <demo> status`. The
    // project's trust-lsp.toml depends on libraries/plcopen_motion/single_axis_core, so that tree
    // is copied alongside the example.
    let runtime = ExampleRuntime::new("plcopen_motion_single_axis_demo")
        .library("libraries/plcopen_motion/single_axis_core")
        .start();
    runtime.wait_until(WAIT, "status to report running", || {
        runtime.status().state == "running"
    });
    assert_eq!(runtime.status().fault, "none", "{}", runtime.diagnostics());
}

#[test]
fn plc_quick_start_shutdown_stops_the_plc() {
    // docs/guides/PLC_QUICK_START.md section 3: `trust-runtime ctl --project . shutdown`. The copy
    // stands in for `.`.
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    runtime.wait_for_snapshot(WAIT, "a running scan", |_| true);

    runtime.ctl_ok(&["shutdown"]);
    let exit = runtime.wait_for_exit(Duration::from_secs(10));
    assert!(
        exit.success(),
        "the PLC must exit successfully after shutdown: {exit}\n{}",
        runtime.diagnostics()
    );
}

#[test]
fn plc_operator_guide_status_io_read_restart_and_shutdown() {
    // docs/guides/PLC_OPERATOR_GUIDE.md on examples/memory_marker_counter: `status` (the summary
    // line), `io-read`, `stats` (for cycle timing per task), `restart cold` and `shutdown`. The
    // restart and the shutdown both run even if the restart is refused, and the refusal is reported
    // after the shutdown has been attempted.
    let mut runtime = ExampleRuntime::new("memory_marker_counter").start();
    runtime.wait_until(WAIT, "status to report running", || {
        runtime.status().state == "running"
    });
    assert_eq!(runtime.status().fault, "none", "{}", runtime.diagnostics());
    runtime.wait_for_snapshot(WAIT, "the first I/O snapshot", |_| true);
    runtime.ctl_ok(&["io-read"]);
    runtime.wait_until(WAIT, "a task line from `stats`", || {
        runtime
            .ctl_ok(&["stats"])
            .lines()
            .any(|line| line.starts_with("task="))
    });

    let restart = runtime.ctl(&["restart", "cold"]);
    let shutdown = runtime.ctl(&["shutdown"]);
    let exit = runtime.wait_for_exit(Duration::from_secs(10));

    assert!(
        restart.status.success(),
        "the guide's `restart cold` was refused: {}",
        stderr_text(&restart)
    );
    assert!(
        shutdown.status.success(),
        "the guide's `shutdown` was refused: {}",
        stderr_text(&shutdown)
    );
    assert!(
        exit.success(),
        "the PLC must exit successfully after shutdown: {exit}\n{}",
        runtime.diagnostics()
    );
}

#[test]
fn plc_io_binding_guide_io_read_and_output_write_are_accepted() {
    // docs/guides/PLC_IO_BINDING_GUIDE.md section 8: `io-read`, then `io-write %QX0.0 TRUE`. The
    // mitsubishi example binds %QX0.0, but its program assigns that output on every scan, and the
    // guide promises no read-back. The test therefore checks that the documented write is accepted.
    let runtime = ExampleRuntime::new("mitsubishi_gxworks3_v1").start();
    runtime.wait_for_snapshot(WAIT, "the first I/O snapshot", |_| true);

    runtime.ctl_ok(&["io-read"]);
    let queued = runtime.ctl_ok(&["io-write", "%QX0.0", "TRUE"]);
    assert!(queued.contains("queued"), "{queued}");
}

#[test]
fn runtime_cloud_troubleshooting_checks_run_as_written() {
    // docs/guides/RUNTIME_CLOUD_TROUBLESHOOTING.md section 1: `ctl --project <project> status`,
    // then `ctl --project <project> config-get`. The config must report the vm execution backend.
    let runtime = ExampleRuntime::new("memory_marker_counter").start();
    runtime.wait_until(WAIT, "status to report running", || {
        runtime.status().state == "running"
    });
    assert_eq!(runtime.status().fault, "none", "{}", runtime.diagnostics());
    assert_reports_vm_backend(&runtime, "docs/guides/RUNTIME_CLOUD_TROUBLESHOOTING.md");
}

#[test]
fn runtime_cloud_profile_cookbook_config_get_runs_as_written() {
    // docs/guides/RUNTIME_CLOUD_PROFILE_COOKBOOK.md section 7: `ctl --project <project>
    // config-get`. The cookbook shows no status command.
    let runtime = ExampleRuntime::new("memory_marker_counter").start();
    assert_reports_vm_backend(&runtime, "docs/guides/RUNTIME_CLOUD_PROFILE_COOKBOOK.md");
}

/// A document's `ctl --project <project> config-get`: the result must report the vm execution
/// backend (config_handlers_get.rs, `runtime.execution_backend`).
fn assert_reports_vm_backend(runtime: &Runtime, document: &str) {
    let config = runtime.ctl_result(&["config-get"]);
    assert_eq!(
        config["runtime.execution_backend"], "vm",
        "{document} must report the vm backend: {config}"
    );
}

#[test]
fn every_documented_ctl_command_is_accepted_by_the_cli() {
    // The check must have teeth: first it refuses the mistakes it exists to catch and accepts the
    // corrected forms. clap refuses an unknown flag or subcommand with a non-zero exit before it
    // prints help, so `ctl <arguments> --help` succeeds only for arguments the CLI accepts.
    for refused in [&["restart", "--mode", "cold"][..], &["config.get"][..]] {
        assert!(
            !cli_accepts_help(refused),
            "the check must refuse `ctl {}`",
            refused.join(" ")
        );
    }
    for accepted in [&["restart", "cold"][..], &["io-write", "%MW0", "41"][..]] {
        assert!(
            cli_accepts_help(accepted),
            "the check must accept `ctl {}`",
            accepted.join(" ")
        );
    }

    // Every guide, and every README under examples/, recursively.
    let root = repo_root();
    let mut documents: Vec<PathBuf> = sorted_entries(&root.join("docs").join("guides"))
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    readme_files(&root.join("examples"), &mut documents);
    assert!(
        !documents.is_empty(),
        "no guides or example READMEs were found"
    );

    let mut verdicts: HashMap<Vec<String>, bool> = HashMap::new();
    let mut invocations = 0;
    let mut offending = Vec::new();
    for document in &documents {
        let bytes =
            fs::read(document).unwrap_or_else(|err| panic!("read {}: {err}", document.display()));
        let text = String::from_utf8_lossy(&bytes);
        let shown = document
            .strip_prefix(&root)
            .unwrap_or(document.as_path())
            .display()
            .to_string();
        for (index, line) in text.lines().enumerate() {
            let Some(arguments) = documented_ctl_arguments(line) else {
                continue;
            };
            invocations += 1;
            if arguments.is_empty() {
                offending.push(format!(
                    "{shown}:{}: trust-runtime ctl (no subcommand)",
                    index + 1
                ));
                continue;
            }
            let accepted = *verdicts.entry(arguments.clone()).or_insert_with(|| {
                let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
                cli_accepts_help(&words)
            });
            if !accepted {
                offending.push(format!(
                    "{shown}:{}: trust-runtime ctl {}",
                    index + 1,
                    arguments.join(" ")
                ));
            }
        }
    }
    assert!(invocations > 0, "no documented ctl invocations were found");
    assert!(
        offending.is_empty(),
        "documented ctl commands the CLI rejects:\n{}",
        offending.join("\n")
    );
}

/// The arguments of a `trust-runtime ctl` invocation on this line: the text after the command, up
/// to the first backtick or the end of the line, without the project, endpoint, token and bundle
/// options and their values. None when the line carries no command, as in the prose "Use the
/// `trust-runtime ctl` for stepping".
fn documented_ctl_arguments(line: &str) -> Option<Vec<String>> {
    let (_, after) = line.split_once("trust-runtime ctl")?;
    let text = after.split('`').next().unwrap_or("");
    if text.trim().is_empty() {
        return None;
    }
    let mut tokens = text.split_whitespace();
    let mut arguments = Vec::new();
    while let Some(token) = tokens.next() {
        match token {
            "--project" | "--endpoint" | "--token" | "--bundle" => {
                tokens.next();
            }
            _ => arguments.push(token.to_string()),
        }
    }
    Some(arguments)
}

/// Whether the CLI accepts `ctl <words> --help`. The command contacts no runtime.
fn cli_accepts_help(words: &[&str]) -> bool {
    trust_runtime_command()
        .arg("ctl")
        .args(words)
        .arg("--help")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Every README.md under `dir`, recursively. Hidden directories and build outputs are skipped.
fn readme_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in sorted_entries(dir) {
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        if entry.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                readme_files(&entry, found);
            }
        } else if name == "README.md" {
            found.push(entry);
        }
    }
}

/// The entries of a directory, sorted.
fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    entries
}
