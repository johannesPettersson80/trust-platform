//! `trust-runtime simulate` (specification 11 §6.8.1, "Simulator for machine tests"; specification
//! 48 RU-58): a project's program on a clock that only its test moves, which answers the control
//! protocol on its standard input and output, one JSON object per line each way.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

/// A resource that runs a cycle every 10 ms, halts on a fault, and writes no retain file.
const RUNTIME_TOML: &str = "[bundle]
version = 1

[resource]
name = \"Probe\"
cycle_interval_ms = 10

[runtime.control]
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

/// The project's I/O: the simulated driver, which a simulator does not start either.
const IO_TOML: &str = "[io]
driver = \"simulated\"
params = {}
";

/// Counts its cycles, and divides by zero in its third.
const MAIN_ST: &str = "
PROGRAM Main
VAR
    counter : DINT := 0;
    quotient : DINT := 0;
END_VAR
counter := counter + 1;
IF counter = 3 THEN
    quotient := 100 / (counter - 3);
END_IF;
END_PROGRAM
";

fn trust_runtime() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides trust-runtime to its integration tests"),
    )
}

fn project() -> PathBuf {
    let project = trust_test_support::unique_temp_path("simulate-command");
    std::fs::create_dir_all(project.join("src")).expect("create the project");
    std::fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    std::fs::write(project.join("io.toml"), IO_TOML).expect("write io.toml");
    std::fs::write(project.join("src/main.st"), MAIN_ST).expect("write main.st");
    let built = trust_runtime()
        .arg("build")
        .arg("--project")
        .arg(&project)
        .output()
        .expect("run trust-runtime build");
    assert!(
        built.status.success(),
        "trust-runtime build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    project
}

struct Simulator {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Simulator {
    fn start(project: &Path) -> Self {
        let mut child = trust_runtime()
            .arg("simulate")
            .arg("--project")
            .arg(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start trust-runtime simulate");
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().expect("the simulator's output"));
        Self {
            child,
            input,
            output,
        }
    }

    /// Sends `request` as one line and reads the one line that answers it.
    fn call(&mut self, request: &Value) -> Value {
        let input = self.input.as_mut().expect("the simulator's input");
        writeln!(input, "{request}").expect("write a request");
        input.flush().expect("flush the request");
        let mut line = String::new();
        self.output.read_line(&mut line).expect("read a response");
        serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("a response line that is not JSON ({error}): {line:?}"))
    }

    fn counter(&mut self, id: u64) -> Value {
        let reply = self.call(
            &json!({"id": id, "type": "variables.read", "params": {"paths": ["Main.counter"]}}),
        );
        assert_eq!(reply["ok"], json!(true), "{reply}");
        reply["result"]["values"][0]["value"].clone()
    }
}

#[test]
fn a_simulator_runs_the_cycles_its_test_asks_for_and_ends_with_its_input() {
    // The program counts its cycles on a 10 ms resource and divides by zero in its third, and the
    // project halts on a fault. Worked by hand: 2 cycles run at 0 and 10 ms, so the time is 10 ms
    // and the counter 2; an advance of 5 cycles runs the third at 20 ms, which faults, so it ends
    // there with 1 cycle, the resource faulted and the fault in the answer; the counter keeps the
    // 3 that the faulting scan wrote before its division. The process ends when its input closes,
    // with success. Catches a simulator that answers on another stream or with a line that is not
    // JSON, one that runs on the wall clock, an advance that runs past a fault or hides it, and a
    // process that does not end with its input.
    let project = project();
    let mut simulator = Simulator::start(&project);

    let advanced =
        simulator.call(&json!({"id": 1, "type": "simulator.advance", "params": {"cycles": 2}}));
    assert_eq!(advanced["ok"], json!(true), "{advanced}");
    assert_eq!(
        advanced["result"],
        json!({"time_ns": 10_000_000, "cycles": 2, "state": "running"})
    );
    assert_eq!(simulator.counter(2), json!(2));

    let faulted =
        simulator.call(&json!({"id": 3, "type": "simulator.advance", "params": {"cycles": 5}}));
    assert_eq!(faulted["ok"], json!(true), "{faulted}");
    assert_eq!(faulted["result"]["time_ns"], json!(20_000_000));
    assert_eq!(faulted["result"]["cycles"], json!(1));
    assert_eq!(faulted["result"]["state"], json!("faulted"));
    assert!(faulted["result"]["fault"].is_object(), "{faulted}");
    assert_eq!(simulator.counter(4), json!(3));

    drop(simulator.input.take());
    let status = simulator.child.wait().expect("the simulator ends");
    assert!(status.success(), "{status}");
    let _ = std::fs::remove_dir_all(project);
}
