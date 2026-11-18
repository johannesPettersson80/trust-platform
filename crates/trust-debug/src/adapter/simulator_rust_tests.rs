//! The compiled simulator of a project with Rust POUs (specification 48 RU-76 with specification 44
//! NX-16): a launch builds the project's crate as `trust-runtime build` does and debugs the program
//! that calls it. Its control server listens on TCP, so the test runs on every computer whose
//! toolchain at `TRUST_TOOLCHAIN` carries cargo and the SDK's sources.

use super::remote::{RemoteEndpoint, RemoteSession};
use super::*;
use serde_json::json;
use std::path::PathBuf;
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
        std::fs::write(root.join("rust/src/lib.rs"), CRATE).expect("the crate");
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
    succeeded(&adapter.dispatch_request(request(8, "disconnect", json!({}))));
    assert!(adapter.simulator.is_none());
}
