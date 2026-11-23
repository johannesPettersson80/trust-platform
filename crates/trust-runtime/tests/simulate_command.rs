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

/// Counts its cycles, stamps each with `CURRENT_DT()`, and divides by zero in its third.
const MAIN_ST: &str = "
PROGRAM Main
VAR
    counter : DINT := 0;
    quotient : DINT := 0;
    stamp : DT;
END_VAR
counter := counter + 1;
stamp := CURRENT_DT();
IF counter = 3 THEN
    quotient := 100 / (counter - 3);
END_IF;
END_PROGRAM
";

/// A sequence that a tool starts with a method of its interface (specification 11 §6.8.1, "Calling
/// a method"), whose `Fail` method divides by zero, and a program that copies its state.
const SEQUENCE_ST: &str = "
INTERFACE ISequence
METHOD Start : BOOL
VAR_INPUT Setpoint : REAL; END_VAR
END_METHOD
METHOD Fail
END_METHOD
END_INTERFACE

INTERFACE IReset
METHOD Reset
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Sequence IMPLEMENTS ISequence
VAR_OUTPUT running : BOOL; target : REAL; END_VAR
VAR divisor : DINT := 0; quotient : DINT; END_VAR
METHOD PUBLIC Start : BOOL
VAR_INPUT Setpoint : REAL; END_VAR
    running := TRUE;
    target := Setpoint;
    Start := TRUE;
END_METHOD
METHOD PUBLIC Fail
    quotient := 100 / divisor;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Heating : Sequence;
    running : BOOL;
    target : REAL;
END_VAR
Heating();
running := Heating.running;
target := Heating.target;
END_PROGRAM
";

fn trust_runtime() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides trust-runtime to its integration tests"),
    )
}

fn project() -> PathBuf {
    project_with(MAIN_ST)
}

/// A built project of the resource above whose program is `main_st`.
fn project_with(main_st: &str) -> PathBuf {
    let project = trust_test_support::unique_temp_path("simulate-command");
    std::fs::create_dir_all(project.join("src")).expect("create the project");
    std::fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    std::fs::write(project.join("io.toml"), IO_TOML).expect("write io.toml");
    std::fs::write(project.join("src/main.st"), main_st).expect("write main.st");
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
        self.read(id, "Main.counter")
    }

    /// The value of the variable at `path` that the last completed scan left.
    fn read(&mut self, id: u64, path: &str) -> Value {
        let reply =
            self.call(&json!({"id": id, "type": "variables.read", "params": {"paths": [path]}}));
        assert_eq!(reply["ok"], json!(true), "{reply}");
        reply["result"]["values"][0]["value"].clone()
    }

    /// Runs `cycles` cycles and answers the advance's result.
    fn advance(&mut self, id: u64, cycles: u64) -> Value {
        let reply = self
            .call(&json!({"id": id, "type": "simulator.advance", "params": {"cycles": cycles}}));
        assert_eq!(reply["ok"], json!(true), "{reply}");
        reply["result"].clone()
    }

    /// Calls `method` of interface `interface` of `Main.Heating` with `inputs`.
    fn call_method(&mut self, id: u64, interface: &str, method: &str, inputs: Value) -> Value {
        self.call(&json!({"id": id, "type": "method.call", "params": {
            "instance": "Main.Heating", "interface": interface, "method": method, "inputs": inputs
        }}))
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
    // `CURRENT_DT()` is the logical time from 1970 (specification 11 §6.8.1): the cycle at 10 ms reads
    // DT#1970-01-01-00:00:00.010, 10 ticks of the default 1 ms resolution (specification 10).
    let stamp = simulator
        .call(&json!({"id": 20, "type": "variables.read", "params": {"paths": ["Main.stamp"]}}));
    assert_eq!(stamp["result"]["values"][0]["value"], json!(10), "{stamp}");

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

#[test]
fn a_tool_calls_a_method_between_scans_and_a_faulting_method_faults_the_resource() {
    // Specification 11 §6.8.1, "Calling a method", on the 10 ms resource above, which halts on a
    // fault. Worked by hand: the cycle at 0 ms runs the sequence that nothing started, so
    // `Main.running` is FALSE. `Start` with a set point of 21.5, a literal typed by the input's
    // declared REAL as `set` types one, answers TRUE between that scan and the next, and the cycle
    // at 10 ms copies running TRUE and the target 21.5 into the program. An input `Start` does not
    // have, and an interface the sequence does not implement, are the kernel's refusals, and the
    // resource runs on: its next cycle is at 20 ms. `Fail` divides by zero: the request is refused with the fault, and
    // the resource faults as at a faulted scan, so a further call is refused as a pause is
    // (`resource_not_running`). Catches a call made inside a scan or not at all, an input not
    // passed, a result not answered, a refusal that stops the resource, and a fault that leaves the
    // resource running.
    let project = project_with(SEQUENCE_ST);
    let mut simulator = Simulator::start(&project);

    assert_eq!(
        simulator.advance(1, 1),
        json!({"time_ns": 0, "cycles": 1, "state": "running"})
    );
    assert_eq!(simulator.read(2, "Main.running"), json!(false));

    let started = simulator.call_method(3, "ISequence", "Start", json!({"Setpoint": "21.5"}));
    assert_eq!(started["ok"], json!(true), "{started}");
    assert_eq!(started["result"], json!({"value": true}));
    assert_eq!(
        simulator.advance(4, 1),
        json!({"time_ns": 10_000_000, "cycles": 1, "state": "running"})
    );
    assert_eq!(simulator.read(5, "Main.running"), json!(true));
    assert_eq!(simulator.read(6, "Main.target"), json!(21.5));

    let unknown = simulator.call_method(11, "ISequence", "Start", json!({"Speed": "3"}));
    assert_eq!(unknown["ok"], json!(false), "{unknown}");
    assert_eq!(unknown["error"], json!("invalid argument name 'Speed'"));
    let refused = simulator.call_method(7, "IReset", "Reset", json!({}));
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(
        refused["error"],
        json!("control error 'Main.Heating does not implement IReset'")
    );
    assert_eq!(
        simulator.advance(8, 1),
        json!({"time_ns": 20_000_000, "cycles": 1, "state": "running"})
    );

    let failed = simulator.call_method(9, "ISequence", "Fail", json!({}));
    assert_eq!(failed["ok"], json!(false), "{failed}");
    assert_eq!(
        failed["error"],
        json!("the method faulted: division by zero")
    );
    let after = simulator.call_method(10, "ISequence", "Start", json!({"Setpoint": "5.0"}));
    assert_eq!(after["ok"], json!(false), "{after}");
    assert_eq!(
        after["error"],
        json!("resource is faulted; restart required")
    );
    assert_eq!(after["error_code"], json!("resource_not_running"));

    drop(simulator.input.take());
    let status = simulator.child.wait().expect("the simulator ends");
    assert!(status.success(), "{status}");
    let _ = std::fs::remove_dir_all(project);
}
