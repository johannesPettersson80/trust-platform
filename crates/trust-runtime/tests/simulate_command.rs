//! `trust-runtime simulate` (specification 11 §6.8.1, "Simulator for machine tests"; specification
//! 48 RU-58): a project's program on a clock that only its test moves, which answers the control
//! protocol on its standard input and output, one JSON object per line each way.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_runtime_core::native::image::{Image, StmtRecord, SymbolRecord};
use trust_runtime_core::trace::{
    decode, Activation, Change, ChangeKind, ChangeSource, CycleFault, IoArea, Located, Trace,
    TraceEnd, TraceOrigin,
};

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

/// A task that runs at every 10 ms cycle, a global that the test writes, a located input, and a
/// located output that copies the global; the program reads `CURRENT_DT()` once a cycle and divides
/// by `limit` once it is 0.
const RECORDED_ST: &str = "
VAR_GLOBAL
    limit : INT := 5;
    start AT %IX0.0 : BOOL;
    level AT %QW0 : INT;
END_VAR

PROGRAM Main
VAR
    stamp : DT;
    quotient : INT;
END_VAR
stamp := CURRENT_DT();
level := limit;
IF limit = 0 THEN
    quotient := 100 / limit;
END_IF;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// A simulation whose disturbance faults the resource before the cycle at 20 ms (specification 46).
const FAULT_AT_20_MS: &str = r#"[simulation]
enabled = true

[[disturbances]]
at_ms = 20
kind = "fault"
message = "scripted fault"
"#;

/// A simulation whose disturbance sets the input `start` (%IX0.0) before the cycle at 20 ms
/// (specification 46).
const START_AT_20_MS: &str = r#"[simulation]
enabled = true

[[disturbances]]
at_ms = 20
kind = "set"
target = "%IX0.0"
value = "TRUE"
"#;

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

    /// Sets the global `name` to `value` before the next scan (`set`).
    fn set(&mut self, id: u64, name: &str, value: &str) {
        let queued = self.call(&json!({"id": id, "type": "set", "params": {
            "target": format!("global:{name}"), "value": value
        }}));
        assert_eq!(queued["ok"], json!(true), "{queued}");
    }

    /// The ended trace `trace`'s state, and its file read part by part from request `id` on, each
    /// part's offset, CRC-32 and next offset checked, and the whole file's SHA-256 against the
    /// state's.
    fn trace(&mut self, id: u64, trace: u64) -> (Value, Vec<u8>) {
        let state = self.call(&json!({"id": id, "type": "trace.read", "params": {"trace": trace}}));
        assert_eq!(state["ok"], json!(true), "{state}");
        let state = state["result"].clone();
        let bytes = state["bytes"].as_u64().expect("an ended trace's bytes");
        let mut file = Vec::new();
        let mut request = id;
        while u64::try_from(file.len()).unwrap() < bytes {
            request += 1;
            let part = self.call(&json!({"id": request, "type": "trace.read", "params": {
                "trace": trace, "offset": file.len()
            }}));
            assert_eq!(part["ok"], json!(true), "{part}");
            let part = &part["result"];
            assert_eq!(part["offset"], json!(file.len()));
            let data = base64::engine::general_purpose::STANDARD
                .decode(part["data"].as_str().expect("a part's data"))
                .expect("a part's data is base64");
            assert!(!data.is_empty() && data.len() <= 4096, "{part}");
            assert_eq!(
                part["crc32"],
                json!(format!("{:08x}", crc32fast::hash(&data)))
            );
            file.extend_from_slice(&data);
            assert_eq!(part["next_offset"], json!(file.len()));
        }
        assert_eq!(state["sha256"], json!(hex(&Sha256::digest(&file))));
        (state, file)
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decoded(file: &[u8]) -> Trace {
    let decoded = decode(file).expect("the trace decodes");
    assert_eq!(decoded.damaged_cycle, None);
    decoded.trace
}

/// The control link's write of the INT global `limit`: its bytes, little-endian.
fn limit_written(value: i16) -> Change {
    Change {
        source: ChangeSource::ControlLink,
        kind: ChangeKind::VariableWrite {
            path: "limit".into(),
            type_name: "INT".into(),
            value: value.to_le_bytes().to_vec(),
        },
    }
}

#[test]
fn a_recording_keeps_each_cycle_with_its_changes_clock_readings_and_images() {
    // Specification 11 §6.8.1, "Cycle traces", and specification 48 RU-59 with Appendix C, on the
    // 10 ms resource above. Worked by hand: the cycle at 0 ms, the runtime's cycle 0, copies 5 into
    // `level` (%QW0). `trace.record` for 2 cycles answers trace 1; until its first cycle a second
    // recording is refused `trace_busy` and the first reads as recording 0 cycles. A `set` of
    // `limit` to 7, queued now, is the next cycle's change. The advance of 3 cycles runs the cycles
    // at 10, 20 and 30 ms; the recording takes the runtime's cycles 1 and 2 and ends `complete`.
    // Its start state is the memory before the write: `limit` still 5 where the image's symbol table
    // places it, and %QW0 still 5. Its cycle at 10 ms holds the write (the control link's, `limit`,
    // INT, the bytes 07 00), task 0's activation at 10 ms, the reading of `CURRENT_DT` at 10 ms from
    // 1970, the input image of one byte (%IX0.0 FALSE) and %QW0 at 7; its cycle at 20 ms the same at
    // 20 ms without a change. Its located variables are `start` (%IX0.0, a bit of one byte) and
    // `level` (%QW0, 2 bytes). The file, read in parts, names the running image
    // (program.tpi's SHA-256, its sources' hash at offset 32 and its target at offset 20,
    // specification 44 §5.2) and the runtime as `status` does. Catches a recording of the cycle
    // before it was asked for or past its goal, a change or reading lost or put in another cycle, a
    // start state taken after the cycle's writes, images of another scan, a file whose parts or
    // identity do not match, and a trace still readable after the next recording started.
    let project = project_with(RECORDED_ST);
    let image = std::fs::read(project.join("program.tpi")).expect("read the built image");
    let mut simulator = Simulator::start(&project);
    assert_eq!(
        simulator.advance(1, 1),
        json!({"time_ns": 0, "cycles": 1, "state": "running"})
    );

    let started =
        simulator.call(&json!({"id": 2, "type": "trace.record", "params": {"cycles": 2}}));
    assert_eq!(started["ok"], json!(true), "{started}");
    assert_eq!(started["result"]["trace"], json!(1));
    assert!(
        started["result"]["reserved_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0),
        "{started}"
    );
    let busy = simulator.call(&json!({"id": 3, "type": "trace.record", "params": {"cycles": 1}}));
    assert_eq!(busy["error_code"], json!("trace_busy"), "{busy}");
    let waiting = simulator.call(&json!({"id": 4, "type": "trace.read", "params": {"trace": 1}}));
    assert_eq!(
        waiting["result"],
        json!({"state": "recording", "cycles": 0}),
        "{waiting}"
    );
    simulator.set(5, "limit", "7");
    assert_eq!(
        simulator.advance(6, 3),
        json!({"time_ns": 30_000_000, "cycles": 3, "state": "running"})
    );

    let (state, file) = simulator.trace(7, 1);
    assert_eq!(state["state"], json!("ended"));
    assert_eq!(state["reason"], json!("complete"));
    assert_eq!(state["cycles"], json!(2));
    let status = simulator.call(&json!({"id": 30, "type": "status"}));
    let trace = decoded(&file);
    let meta = &trace.meta;
    assert_eq!(
        meta.image_sha256.as_slice(),
        Sha256::digest(&image).as_slice()
    );
    assert_eq!(meta.sources_hash.as_slice(), &image[32..64]);
    assert_eq!(
        meta.target,
        u32::from_le_bytes(image[20..24].try_into().unwrap())
    );
    assert_eq!(meta.end, TraceEnd::Complete);
    assert_eq!(meta.cycle_interval_ns, 10_000_000);
    assert_eq!((meta.first_cycle, meta.cycle_count), (1, 2));
    assert_eq!(meta.runtime_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(json!(meta.runtime_id), status["result"]["runtime_id"]);
    assert_eq!(json!(meta.platform), status["result"]["platform"]);
    assert_eq!(meta.resource, "Probe");
    assert_eq!(meta.task, None);
    assert_eq!(meta.origin, TraceOrigin::OnDemand);
    let mut located = trace.located.clone();
    located.sort_by(|left, right| left.path.cmp(&right.path));
    assert_eq!(
        located,
        [
            Located {
                path: "level".into(),
                type_name: "INT".into(),
                area: IoArea::Output,
                bit: None,
                offset: 0,
                size: 2,
            },
            Located {
                path: "start".into(),
                type_name: "BOOL".into(),
                area: IoArea::Input,
                bit: Some(0),
                offset: 0,
                size: 1,
            },
        ]
    );
    let start_bytes = |area: u8, offset: u32| {
        let (_, bytes) = trace
            .start
            .areas
            .iter()
            .find(|(id, _)| *id == u32::from(area))
            .expect("the area's start bytes");
        let at = usize::try_from(offset).unwrap();
        bytes[at..at + 2].to_vec()
    };
    let parsed = Image::parse(&image).expect("the built image parses");
    let limit = parsed
        .table::<SymbolRecord>()
        .expect("a symbol table")
        .iter()
        .find(|symbol| parsed.string(symbol.display) == Some("limit"))
        .expect("the symbol limit");
    assert_eq!(start_bytes(limit.area, limit.offset), [5, 0]);
    assert_eq!(start_bytes(2, 0), [5, 0]);
    assert!(trace.start.forces.is_empty());
    assert_eq!(trace.cycles.len(), 2);
    for (cycle, (time_ns, changes)) in trace
        .cycles
        .iter()
        .zip([(10_000_000, vec![limit_written(7)]), (20_000_000, vec![])])
    {
        assert_eq!(cycle.time_ns, time_ns);
        assert_eq!(
            cycle.activations,
            [Activation {
                task: 0,
                now_ns: time_ns
            }]
        );
        assert_eq!(cycle.wall_clock_ms, [time_ns / 1_000_000]);
        assert_eq!(cycle.changes, changes);
        assert_eq!(cycle.inputs, [0]);
        assert_eq!(&cycle.outputs[..2], &[7, 0]);
        assert_eq!(cycle.fault, None);
    }

    // An ended trace is answered as it ended, also by a stop; a part past its end is refused.
    let stopped = simulator.call(&json!({"id": 31, "type": "trace.stop", "params": {"trace": 1}}));
    assert_eq!(stopped["result"], state, "{stopped}");
    let past = simulator.call(&json!({"id": 32, "type": "trace.read", "params": {
        "trace": 1, "offset": file.len()
    }}));
    assert_eq!(past["ok"], json!(false), "{past}");
    // A started recording replaces the last one's trace (specification 11, "Cycle traces").
    let next = simulator.call(&json!({"id": 33, "type": "trace.record", "params": {"cycles": 1}}));
    assert_eq!(next["result"]["trace"], json!(2), "{next}");
    let replaced = simulator.call(&json!({"id": 34, "type": "trace.read", "params": {"trace": 1}}));
    assert_eq!(replaced["error_code"], json!("unknown_trace"), "{replaced}");

    drop(simulator.input.take());
    let status = simulator.child.wait().expect("the simulator ends");
    assert!(status.success(), "{status}");
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn a_recording_ends_when_stopped_and_with_the_fault_of_its_last_cycle() {
    // Specification 11 §6.8.1, "Cycle traces", and specification 48 RU-59, on the resource above,
    // which halts on a fault. Worked by hand: after the cycle at 0 ms, trace 1 for 10 cycles records
    // the cycle at 10 ms, and `trace.stop` ends it `stopped` with that 1 cycle. Trace 2, with `limit`
    // set to 0 before the cycle at 20 ms, ends with that cycle `faulted`: the division by zero, code
    // 1 (specification 44, Appendix A), at the statement `quotient := 100 / limit;` and its POU, as
    // the image's statement table names them; the cycle keeps its task's activation, its reading of
    // the clock (20 ms, read before the division), the write, and the input image its scan sampled:
    // %IX0.0 TRUE, which the simulation's disturbance at 20 ms set (specification 46). The
    // faulted resource then refuses a recording as it refuses a pause (`resource_not_running`).
    // Catches a stop that waits for the goal, a faulted cycle dropped or recorded without its
    // fault's place, and a recording started on a resource that does not run.
    let project = project_with(RECORDED_ST);
    std::fs::write(project.join("simulation.toml"), START_AT_20_MS).expect("write simulation.toml");
    let image = std::fs::read(project.join("program.tpi")).expect("read the built image");
    let mut simulator = Simulator::start(&project);
    assert_eq!(
        simulator.advance(1, 1),
        json!({"time_ns": 0, "cycles": 1, "state": "running"})
    );

    let first = simulator.call(&json!({"id": 2, "type": "trace.record", "params": {"cycles": 10}}));
    assert_eq!(first["result"]["trace"], json!(1), "{first}");
    simulator.advance(3, 1);
    let stopped = simulator.call(&json!({"id": 4, "type": "trace.stop", "params": {"trace": 1}}));
    assert_eq!(stopped["result"]["state"], json!("ended"), "{stopped}");
    assert_eq!(stopped["result"]["reason"], json!("stopped"));
    assert_eq!(stopped["result"]["cycles"], json!(1));
    let (_, file) = simulator.trace(5, 1);
    let trace = decoded(&file);
    assert_eq!(trace.meta.end, TraceEnd::Stopped);
    assert_eq!(trace.cycles.len(), 1);
    assert_eq!(trace.cycles[0].time_ns, 10_000_000);

    let second =
        simulator.call(&json!({"id": 20, "type": "trace.record", "params": {"cycles": 10}}));
    assert_eq!(second["result"]["trace"], json!(2), "{second}");
    simulator.set(21, "limit", "0");
    let faulted = simulator.advance(22, 2);
    assert_eq!(faulted["state"], json!("faulted"), "{faulted}");
    assert_eq!(faulted["time_ns"], json!(20_000_000));
    let (state, file) = simulator.trace(23, 2);
    assert_eq!(state["reason"], json!("faulted"));
    assert_eq!(state["cycles"], json!(1));
    let trace = decoded(&file);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    let [cycle] = trace.cycles.as_slice() else {
        panic!("one faulted cycle: {:?}", trace.cycles);
    };
    assert_eq!(cycle.time_ns, 20_000_000);
    assert_eq!(
        cycle.activations,
        [Activation {
            task: 0,
            now_ns: 20_000_000
        }]
    );
    assert_eq!(cycle.wall_clock_ms, [20]);
    assert_eq!(cycle.changes, [limit_written(0)]);
    assert_eq!(cycle.inputs, [1]);
    let line = RECORDED_ST
        .lines()
        .position(|line| line.contains("100 / limit"))
        .map(|index| u32::try_from(index + 1).unwrap())
        .expect("the division's line");
    let image = Image::parse(&image).expect("the built image parses");
    let statements = image.table::<StmtRecord>().expect("a statement table");
    let (statement, record) = (0_u32..)
        .zip(statements.iter())
        .find(|(_, record)| record.start_line == line)
        .expect("the division's statement");
    assert_eq!(
        cycle.fault,
        Some(CycleFault {
            code: 1,
            pou: u32::from(record.pou),
            statement,
        })
    );

    let refused =
        simulator.call(&json!({"id": 40, "type": "trace.record", "params": {"cycles": 1}}));
    assert_eq!(
        refused["error_code"],
        json!("resource_not_running"),
        "{refused}"
    );

    drop(simulator.input.take());
    let status = simulator.child.wait().expect("the simulator ends");
    assert!(status.success(), "{status}");
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn a_recording_ends_faulted_when_its_resource_faults_outside_a_scan() {
    // Specification 48 RU-59: a recording ends when its resource faults, with the cycles until
    // then. Worked by hand: after the cycle at 0 ms, trace 1 for 10 cycles records the cycle at
    // 10 ms; the simulation's disturbance then faults the resource before the cycle at 20 ms runs,
    // so the trace ends `faulted` with that 1 cycle, which holds no fault of its own. Catches a
    // recording left running on a faulted resource, as if it could record on.
    let project = project_with(RECORDED_ST);
    std::fs::write(project.join("simulation.toml"), FAULT_AT_20_MS).expect("write simulation.toml");
    let mut simulator = Simulator::start(&project);
    assert_eq!(
        simulator.advance(1, 1),
        json!({"time_ns": 0, "cycles": 1, "state": "running"})
    );
    let started =
        simulator.call(&json!({"id": 2, "type": "trace.record", "params": {"cycles": 10}}));
    assert_eq!(started["result"]["trace"], json!(1), "{started}");
    let faulted = simulator.advance(3, 3);
    assert_eq!(faulted["state"], json!("faulted"), "{faulted}");

    let ended = simulator.call(&json!({"id": 4, "type": "trace.read", "params": {"trace": 1}}));
    assert_eq!(ended["result"]["state"], json!("ended"), "{ended}");
    assert_eq!(ended["result"]["reason"], json!("faulted"));
    assert_eq!(ended["result"]["cycles"], json!(1));
    let (_, file) = simulator.trace(5, 1);
    let trace = decoded(&file);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    let [cycle] = trace.cycles.as_slice() else {
        panic!("the cycle at 10 ms: {:?}", trace.cycles);
    };
    assert_eq!(cycle.time_ns, 10_000_000);
    assert_eq!(cycle.fault, None);

    drop(simulator.input.take());
    let status = simulator.child.wait().expect("the simulator ends");
    assert!(status.success(), "{status}");
    let _ = std::fs::remove_dir_all(project);
}
