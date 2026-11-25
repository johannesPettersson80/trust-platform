//! The harness of the tests of `trust-runtime simulate` (specification 11 §6.8.1, "Simulator for
//! machine tests"; specification 48 RU-58): a built project of a resource whose task runs every
//! 10 ms, and the simulator of it, which answers the control protocol on its standard input and
//! output, one JSON object per line each way, with the helpers of its traces.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trust_runtime_core::trace::{decode, Trace};

/// A resource that runs a cycle every 10 ms, halts on a fault, and writes no retain file.
pub const RUNTIME_TOML: &str = "[bundle]
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
pub const IO_TOML: &str = "[io]
driver = \"simulated\"
params = {}
";

pub fn trust_runtime() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides trust-runtime to its integration tests"),
    )
}

/// A built project of the resource above whose program is `main_st`.
pub fn project_with(main_st: &str) -> PathBuf {
    project_with_io(main_st, IO_TOML)
}

/// A built project of the resource above whose program is `main_st` and whose I/O is `io_toml`.
pub fn project_with_io(main_st: &str, io_toml: &str) -> PathBuf {
    let project = trust_test_support::unique_temp_path("simulate-command");
    std::fs::create_dir_all(project.join("src")).expect("create the project");
    std::fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    std::fs::write(project.join("io.toml"), io_toml).expect("write io.toml");
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

/// A running `trust-runtime simulate`, its standard input and output the control link.
pub struct Simulator {
    /// The simulator's process.
    pub child: Child,
    /// Its standard input, which a test closes to end it.
    pub input: Option<ChildStdin>,
    /// Its standard output, one answer per line.
    pub output: BufReader<ChildStdout>,
}

impl Simulator {
    pub fn start(project: &Path) -> Self {
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
    pub fn call(&mut self, request: &Value) -> Value {
        let input = self.input.as_mut().expect("the simulator's input");
        writeln!(input, "{request}").expect("write a request");
        input.flush().expect("flush the request");
        let mut line = String::new();
        self.output.read_line(&mut line).expect("read a response");
        serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("a response line that is not JSON ({error}): {line:?}"))
    }

    pub fn counter(&mut self, id: u64) -> Value {
        self.read(id, "Main.counter")
    }

    /// The value of the variable at `path` that the last completed scan left.
    pub fn read(&mut self, id: u64, path: &str) -> Value {
        let reply =
            self.call(&json!({"id": id, "type": "variables.read", "params": {"paths": [path]}}));
        assert_eq!(reply["ok"], json!(true), "{reply}");
        reply["result"]["values"][0]["value"].clone()
    }

    /// Runs `cycles` cycles and answers the advance's result.
    pub fn advance(&mut self, id: u64, cycles: u64) -> Value {
        let reply = self
            .call(&json!({"id": id, "type": "simulator.advance", "params": {"cycles": cycles}}));
        assert_eq!(reply["ok"], json!(true), "{reply}");
        reply["result"].clone()
    }

    /// Sends the request `kind` with `params`, asserts that it was answered, and gives its result.
    pub fn request(&mut self, id: u64, kind: &str, params: Value) -> Value {
        let reply = self.call(&json!({"id": id, "type": kind, "params": params}));
        assert_eq!(reply["ok"], json!(true), "{reply}");
        reply["result"].clone()
    }

    /// Sets the global `name` to `value` before the next scan (`set`).
    pub fn set(&mut self, id: u64, name: &str, value: &str) {
        let queued = self.call(&json!({"id": id, "type": "set", "params": {
            "target": format!("global:{name}"), "value": value
        }}));
        assert_eq!(queued["ok"], json!(true), "{queued}");
    }

    /// The ended trace `trace`'s state, and its file read part by part from request `id` on, each
    /// part's offset, CRC-32 and next offset checked, and the whole file's SHA-256 against the
    /// state's.
    pub fn trace(&mut self, id: u64, trace: u64) -> (Value, Vec<u8>) {
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
    pub fn call_method(&mut self, id: u64, interface: &str, method: &str, inputs: Value) -> Value {
        self.call(&json!({"id": id, "type": "method.call", "params": {
            "instance": "Main.Heating", "interface": interface, "method": method, "inputs": inputs
        }}))
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn decoded(file: &[u8]) -> Trace {
    let decoded = decode(file).expect("the trace decodes");
    assert_eq!(decoded.damaged_cycle, None);
    decoded.trace
}
