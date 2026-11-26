//! The compiled simulator of a project with Rust POUs (specification 48 RU-76 with specification 44
//! NX-16): a launch builds the project's crate as `trust-runtime build` does and debugs the program
//! that calls it. Its control server listens on TCP, so the test runs on every computer whose
//! toolchain at `TRUST_TOOLCHAIN` carries cargo and the SDK's sources.

use super::remote::{RemoteEndpoint, RemoteSession};
use super::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration as WallDuration, Instant};

/// The project's crate: a Rust function, which the SDK's procedural macros declare (specification
/// 48 §6.1).
const CRATE: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Its input plus one.
#[trust_function(name = \"PLUS_ONE\")]
#[budget(\"1us\")]
pub fn plus_one(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    Ok(a.wrapping_add(1))
}
";

/// The program, `count` counting the scans and `bumped` set by the Rust function; line 8 is the
/// `IF` after the call.
const MAIN: &str = "PROGRAM Main
VAR
    count : INT;
    bumped : DINT;
END_VAR
count := count + 1;
bumped := PLUS_ONE(41);
IF count > 2 THEN
    count := 0;
END_IF
END_PROGRAM
";

/// The control server's token.
const TOKEN: &str = "debug-control-token";

/// A project folder with [`MAIN`] in `src`, the crate [`CRATE`] in `rust` and the declarations the
/// crate's sources give, removed when dropped.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new() -> Self {
        Self::with_crate(CRATE)
    }

    /// The project with the crate `source` in place of [`CRATE`].
    fn with_crate(source: &str) -> Self {
        let root = trust_test_support::unique_temp_path("trust-simulator-rust");
        std::fs::create_dir_all(root.join("src")).expect("the sources' folder");
        std::fs::create_dir_all(root.join("rust/src")).expect("the crate's folder");
        std::fs::write(root.join("src/main.st"), MAIN).expect("the program");
        std::fs::write(
            root.join("rust/Cargo.toml"),
            format!(
                "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                 [dependencies]\ntrust-plc = \"={}\"\n",
                env!("CARGO_PKG_VERSION")
            ),
        )
        .expect("the crate's manifest");
        std::fs::write(root.join("rust/src/lib.rs"), source).expect("the crate");
        let written = trust_runtime::native::rust_pous::write_declarations(&root);
        assert!(
            matches!(
                written,
                Ok(trust_runtime::native::rust_pous::Written::Wrote(1))
            ),
            "{written:?}"
        );
        Self { root }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn request(seq: u32, command: &str, arguments: Value) -> Request<Value> {
    Request {
        seq,
        message_type: crate::protocol::MessageType::Request,
        command: command.to_string(),
        arguments: Some(arguments),
    }
}

/// [`CRATE`] with a type error on its line 10, `    let wrong: u8 = "text";`: `"text"` starts after
/// 20 characters and is 6 long, the compiler's E0308 at columns 21 to 27.
fn crate_with_a_type_error() -> String {
    CRATE.replace(
        "    Ok(a.wrapping_add(1))",
        "    let wrong: u8 = \"text\";\n    Ok(a.wrapping_add(1))",
    )
}

/// The body of the build's diagnostics event among `events` (specification 25 §6.10).
fn build_diagnostics(events: &[Value]) -> &Value {
    &events
        .iter()
        .find(|event| event["event"] == "trustBuildDiagnostics")
        .unwrap_or_else(|| panic!("the build's diagnostics among {events:?}"))["body"]
}

/// The diagnostics event's body of one E0308 at line 10, columns 21 to 27, of the crate in the
/// project at `root`.
fn the_type_error(root: &std::path::Path) -> Value {
    json!({
        "source": "truST (rustc)",
        "diagnostics": [{
            "path": root.join("rust").join("src").join("lib.rs").display().to_string(),
            "severity": "error",
            "code": "E0308",
            "message": "mismatched types",
            "line": 10,
            "column": 21,
            "end_line": 10,
            "end_column": 27,
        }],
    })
}

/// The one response of `outcome`, which succeeded.
fn succeeded(outcome: &DispatchOutcome) -> &Value {
    let response = outcome.responses.last().expect("a response");
    assert!(response["success"].as_bool().unwrap_or(false), "{response}");
    response
}

/// Specification 48 RU-76 with specification 44 NX-16, worked by hand: the launch of a project
/// whose program calls the Rust function `PLUS_ONE` builds the project's crate with the toolchain's
/// cargo and SDK sources, as `trust-runtime build` does, and runs it. The breakpoint on line 8 with
/// the condition `count = 2`, after the call, holds the second scan, where `bumped` is
/// PLUS_ONE(41) = 42. A toolchain without cargo and the SDK's sources refuses the launch with that
/// reason. Catches a launch that builds the project's Structured Text alone, which refuses the Rust
/// POU without its Rust item (RU-23), so that Start and F5 refuse every project with Rust POUs.
#[test]
fn a_launch_builds_the_projects_rust_crate_and_runs_its_pous() {
    let toolchain = trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let project = Project::new();
    let main = project.root.join("src/main.st");
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    adapter.dispatch_request(request(1, "initialize", json!({})));
    // The launch names the project's sources as Start's does: the extension sends its default
    // include pattern (`trust.runtime.sourceIncludePatterns`), which takes the generated
    // declarations in `src/generated/` with the program.
    let launch = adapter.dispatch_request(request(
        2,
        "launch",
        json!({
            "program": main.to_string_lossy(),
            "runtimeRoot": project.root.to_string_lossy(),
            "runtimeIncludeGlobs": [trust_runtime::runtime_sources::DEFAULT_INCLUDE_GLOB],
            "controlEndpoint": "tcp://127.0.0.1:0",
            "controlAuthToken": TOKEN,
        }),
    ));
    assert!(launch.responses.is_empty(), "the launch waits");
    succeeded(&adapter.dispatch_request(request(
        3,
        "setBreakpoints",
        json!({
            "source": {"path": main.to_string_lossy()},
            "breakpoints": [{"line": 8, "condition": "count = INT#2"}],
        }),
    )));
    let configured = adapter.dispatch_request(request(4, "configurationDone", json!({})));
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        let refused = configured
            .responses
            .iter()
            .find(|response| response["command"] == "launch")
            .expect("the launch's answer");
        assert_eq!(refused["success"], false, "{refused}");
        assert!(
            refused["message"]
                .as_str()
                .is_some_and(|message| message.contains("no cargo and SDK sources")),
            "{refused}"
        );
        return;
    }
    assert!(
        configured
            .responses
            .iter()
            .all(|response| response["success"].as_bool().unwrap_or(false)),
        "{:?}",
        configured.responses
    );
    // A build without diagnostics clears the last build's (specification 25 §6.10).
    assert_eq!(
        build_diagnostics(&configured.events),
        &json!({ "source": "truST (rustc)", "diagnostics": [] })
    );
    let addr = adapter
        .simulator
        .as_ref()
        .expect("the program runs compiled")
        .control_server()
        .tcp_addr();
    let mut client = RemoteSession::connect(RemoteEndpoint::Tcp(addr), Some(TOKEN.to_string()))
        .expect("connect");
    // The first launch compiles the SDK, so the hold may take a while on a slow computer.
    let deadline = Instant::now() + WallDuration::from_secs(60);
    while !client.debug_state().expect("debug.state").paused {
        assert!(Instant::now() < deadline, "no hold within 60 s");
        std::thread::sleep(WallDuration::from_millis(20));
    }
    let stack = adapter.dispatch_request(request(5, "stackTrace", json!({"threadId": 1})));
    let top = &succeeded(&stack)["body"]["stackFrames"][0];
    assert_eq!(top["line"], 8, "{top}");
    let answer = adapter.dispatch_request(request(
        6,
        "evaluate",
        json!({"expression": "bumped", "frameId": top["id"].clone()}),
    ));
    assert_eq!(succeeded(&answer)["body"]["result"], "42");
    succeeded(&adapter.dispatch_request(request(7, "continue", json!({"threadId": 1}))));
    // Specification 25 §6.3 and §6.10: an update whose crate does not compile leaves the program
    // as it was, answers with the crate's one error, which Problems shows, and reports it there.
    std::fs::write(
        project.root.join("rust/src/lib.rs"),
        crate_with_a_type_error(),
    )
    .expect("the crate with a type error");
    let reload = adapter.dispatch_request(request(8, "stReload", json!({})));
    let refused = reload.responses.last().expect("the reload's answer");
    assert_eq!(refused["success"], false, "{refused}");
    assert_eq!(
        refused["message"], "Compile failed — 1 error. Open Problems, then try again.",
        "{refused}"
    );
    assert_eq!(
        build_diagnostics(&reload.events),
        &the_type_error(&project.root)
    );
    succeeded(&adapter.dispatch_request(request(9, "disconnect", json!({}))));
    assert!(adapter.simulator.is_none());
}

/// Specification 25 §6.10 with specification 48 RU-44, worked by hand: a launch whose crate does
/// not compile is refused, and the build's event reports the crate's error where it is, at its
/// file's absolute path: E0308 at line 10, columns 21 to 27 ([`crate_with_a_type_error`]). Catches
/// a launch that refuses with the compiler's text alone, so that Problems never shows the error,
/// and one whose event names the file relative to the project.
#[test]
fn a_launch_whose_crate_does_not_compile_reports_its_error_where_it_is() {
    let toolchain = trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let project = Project::with_crate(&crate_with_a_type_error());
    let main = project.root.join("src/main.st");
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    adapter.dispatch_request(request(1, "initialize", json!({})));
    let launch = adapter.dispatch_request(request(
        2,
        "launch",
        json!({
            "program": main.to_string_lossy(),
            "runtimeRoot": project.root.to_string_lossy(),
            "runtimeIncludeGlobs": [trust_runtime::runtime_sources::DEFAULT_INCLUDE_GLOB],
            "controlEndpoint": "tcp://127.0.0.1:0",
            "controlAuthToken": TOKEN,
        }),
    ));
    assert!(launch.responses.is_empty(), "the launch waits");
    let configured = adapter.dispatch_request(request(3, "configurationDone", json!({})));
    let refused = configured
        .responses
        .iter()
        .find(|response| response["command"] == "launch")
        .expect("the launch's answer");
    assert_eq!(refused["success"], false, "{refused}");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.contains("error[E0308]: mismatched types")),
        "{refused}"
    );
    assert_eq!(
        build_diagnostics(&configured.events),
        &the_type_error(&project.root)
    );
    assert!(adapter.simulator.is_none());
}

/// A copy of the example `rust_first_cell`, a Rust-first project whose crate is its own folder
/// (specification 48 RU-65), without what an earlier build left in the example, removed when
/// dropped.
struct RustFirstCell {
    root: PathBuf,
}

impl RustFirstCell {
    fn new() -> Self {
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rust_first_cell");
        let root = trust_test_support::unique_temp_path("trust-simulator-rust-first");
        copy_project(&example, &root);
        Self { root }
    }
}

impl Drop for RustFirstCell {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Copies the project in `from` to `to`, leaving out what a build or a run writes.
fn copy_project(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the copy's folder");
    for item in std::fs::read_dir(from).expect("the example's folder") {
        let item = item.expect("a folder entry");
        let name = item.file_name();
        if matches!(
            name.to_str(),
            Some("build" | "target" | "program.tpi" | "deployment-record.bin" | ".trust-runtime")
        ) {
            continue;
        }
        let target = to.join(&name);
        if item.file_type().expect("its type").is_dir() {
            copy_project(&item.path(), &target);
        } else {
            std::fs::copy(item.path(), &target).expect("a copy");
        }
    }
}

/// Whether the output at `address` reads TRUE in the runtime's I/O state, as Live Values reads it.
fn output_is_true(client: &mut RemoteSession, address: &str) -> bool {
    client.io_state().is_ok_and(|state| {
        state.outputs.iter().any(|entry| {
            entry.address == address && entry.value.to_ascii_lowercase().contains("true")
        })
    })
}

/// Waits up to `limit` until the output at `address` reads TRUE.
fn wait_for_output(client: &mut RemoteSession, address: &str, limit: WallDuration) {
    let deadline = Instant::now() + limit;
    while !output_is_true(client, address) {
        assert!(
            Instant::now() < deadline,
            "{address} not TRUE within {limit:?}"
        );
        std::thread::sleep(WallDuration::from_millis(20));
    }
}

/// Specification 48 RU-69 and specification 25 §3.2, worked from the example's files: Start on a
/// copy of the example `rust_first_cell` launches its generated configuration, builds the project's
/// own crate and runs the press cell, whose sequence lights the Ready lamp (`%QX0.2`, trust.toml) in
/// its first scan. With the part sensor (`%IX0.1`) and Start (`%IX0.0`) written as Live Values
/// writes an input, the sequence leaves READY at the next scan and opens the clamp's valve
/// (`%QX0.0`). Catches a launch that finds no crate in a project whose crate is its own folder, a
/// configuration built without its Rust program, and bindings that do not reach the program.
#[test]
fn a_launch_of_a_rust_first_project_runs_its_crate_and_takes_its_inputs() {
    let toolchain = trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        // The launch is refused, as a_launch_builds_the_projects_rust_crate_and_runs_its_pous proves.
        return;
    }
    let cell = RustFirstCell::new();
    let configuration = cell.root.join("src/generated/trust/configuration.st");
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    adapter.dispatch_request(request(1, "initialize", json!({})));
    let launch = adapter.dispatch_request(request(
        2,
        "launch",
        json!({
            "program": configuration.to_string_lossy(),
            "runtimeRoot": cell.root.to_string_lossy(),
            "runtimeIncludeGlobs": [trust_runtime::runtime_sources::DEFAULT_INCLUDE_GLOB],
            "controlEndpoint": "tcp://127.0.0.1:0",
            "controlAuthToken": TOKEN,
        }),
    ));
    assert!(launch.responses.is_empty(), "the launch waits");
    let configured = adapter.dispatch_request(request(3, "configurationDone", json!({})));
    assert!(
        configured
            .responses
            .iter()
            .all(|response| response["success"].as_bool().unwrap_or(false)),
        "{:?}",
        configured.responses
    );
    let addr = adapter
        .simulator
        .as_ref()
        .expect("the program runs compiled")
        .control_server()
        .tcp_addr();
    let mut client = RemoteSession::connect(RemoteEndpoint::Tcp(addr), Some(TOKEN.to_string()))
        .expect("connect");
    // The first launch compiles the SDK, so the first scan may take a while on a slow computer.
    wait_for_output(&mut client, "%QX0.2", WallDuration::from_secs(60));
    assert!(
        !output_is_true(&mut client, "%QX0.0"),
        "the clamp waits for a part"
    );
    client.io_write("%IX0.1", "TRUE").expect("the part sensor");
    client.io_write("%IX0.0", "TRUE").expect("Start");
    wait_for_output(&mut client, "%QX0.0", WallDuration::from_secs(10));
    succeeded(&adapter.dispatch_request(request(4, "disconnect", json!({}))));
    assert!(adapter.simulator.is_none());
}
