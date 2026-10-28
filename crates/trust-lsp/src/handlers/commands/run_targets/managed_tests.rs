//! The managed runtimes of a fleet root, the outcome of a managed Start or Stop, and the managed
//! logs (specification 25 §5.1 and §6.5).

use std::path::PathBuf;

use serde_json::{json, Value};

use super::super::fake_runtime::{closed_endpoint, FakeRuntime};
use super::answer;

fn temp_root(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("trust-lsp-managed-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a fleet root");
    root
}

fn outcome(lifecycle: &str, response: Value) -> Value {
    answer(
        "managedOutcome",
        json!({ "lifecycle": lifecycle, "name": "cell1", "fleetRoot": "/fleet", "response": response }),
    )
    .expect("an answer")
}

/// A Start succeeds only at `running` and a Stop only at `stopped`; `starting` and `stopping` are no
/// success (specification 25 §5.1). Catches a process that is up but not yet answering reported as
/// started.
#[test]
fn a_lifecycle_succeeds_only_at_its_reached_state() {
    for (lifecycle, status, success) in [
        ("start", "running", true),
        ("start", "starting", false),
        ("start", "stopped", false),
        ("stop", "stopped", true),
        ("stop", "stopping", false),
        ("stop", "running", false),
    ] {
        let answer = outcome(lifecycle, json!({ "status": status, "message": "m" }));
        assert_eq!(answer["success"], success, "{lifecycle} {status}");
        assert_eq!(answer["status"], status);
        assert_eq!(answer["message"], "m");
    }
}

/// The answer's project folder is below the fleet root unless absolute, the runtime is named as the
/// sidebar names it, and no answer says that the tools did not respond. Catches a relative path
/// handed to the token import as it is.
#[test]
fn the_outcome_resolves_the_project_and_reports_a_missing_answer() {
    let answer = outcome(
        "start",
        json!({ "status": "running", "path": "runtimes/cell1", "control_endpoint": "tcp://127.0.0.1:9910" }),
    );
    assert_eq!(
        answer["projectPath"],
        PathBuf::from("/fleet")
            .join("runtimes/cell1")
            .display()
            .to_string()
    );
    assert_eq!(answer["controlEndpoint"], "tcp://127.0.0.1:9910");
    assert_eq!(answer["label"], "cell1 (this computer)");
    let absolute = outcome(
        "start",
        json!({ "status": "running", "path": "/srv/cell1" }),
    );
    assert_eq!(absolute["projectPath"], "/srv/cell1");
    let none = outcome("stop", Value::Null);
    assert_eq!(none["success"], false);
    assert_eq!(none["message"], "The runtime tools didn't respond.");
}

/// A valid `runtime.toml` whose control endpoint is `endpoint`, with the token `t`.
fn runtime_toml(endpoint: &str) -> String {
    format!(
        "[bundle]\nversion = 1\n\n[resource]\nname = \"cell\"\ncycle_interval_ms = 100\n\n\
         [runtime.control]\nendpoint = \"{endpoint}\"\nauth_token = \"t\"\n\n\
         [runtime.log]\nlevel = \"info\"\n\n\
         [runtime.retain]\nmode = \"none\"\nsave_interval_ms = 1000\n\n\
         [runtime.watchdog]\nenabled = false\ntimeout_ms = 5000\naction = \"halt\"\n\n\
         [runtime.fault]\npolicy = \"halt\"\n\n\
         [runtime.mesh]\nenabled = false\nlisten = \"0.0.0.0:5200\"\ntls = false\npublish = []\nsubscribe = {{}}\n"
    )
}

/// The list names each runtime of `fleet.toml` with the state its status reports: `running` when
/// its endpoint answers, `stopped` when nothing answers, and `unavailable` when its status cannot
/// be read (here its project has no `runtime.toml`), never `stopped`. Catches an unreadable status
/// shown as stopped, which enables a Start that cannot work.
#[test]
fn the_managed_runtimes_with_their_states() {
    let root = temp_root("list");
    let runtime = FakeRuntime::answering(json!({ "state": "running" }));
    let stopped = closed_endpoint();
    for (name, endpoint) in [
        ("cell1", runtime.endpoint.as_str()),
        ("cell2", stopped.as_str()),
    ] {
        std::fs::create_dir_all(root.join(name)).expect("a project");
        std::fs::write(root.join(name).join("runtime.toml"), runtime_toml(endpoint))
            .expect("runtime.toml");
    }
    let unread = closed_endpoint();
    std::fs::write(
        root.join("fleet.toml"),
        format!(
            "[[runtime]]\nname = \"cell1\"\npath = \"cell1\"\ncontrol_endpoint = \"{}\"\nweb_port = 18081\n\n\
             [[runtime]]\nname = \"cell2\"\npath = \"cell2\"\ncontrol_endpoint = \"{}\"\nweb_port = 18082\n\n\
             [[runtime]]\nname = \"cell3\"\npath = \"cell3\"\ncontrol_endpoint = \"{}\"\nweb_port = 18083\n",
            runtime.endpoint, stopped, unread
        ),
    )
    .expect("fleet.toml");
    let answer = answer("managed", json!({ "fleetRoot": root })).expect("an answer");
    let runtimes = answer["runtimes"].as_array().expect("runtimes").clone();
    let state = |name: &str| {
        runtimes
            .iter()
            .find(|runtime| runtime["name"] == name)
            .map(|runtime| runtime["state"].clone())
    };
    assert_eq!(state("cell1"), Some(json!("running")), "{answer}");
    assert_eq!(state("cell2"), Some(json!("stopped")), "{answer}");
    assert_eq!(state("cell3"), Some(json!("unavailable")), "{answer}");
    assert_eq!(runtimes[0]["label"], "cell1 (this computer)");
    assert_eq!(runtimes[0]["controlEndpoint"], runtime.endpoint);
    assert_eq!(
        runtimes[0]["projectPath"],
        root.join("cell1").display().to_string()
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A fleet root whose manifest cannot be read has no managed runtimes and says why. Catches a
/// broken manifest that fails the sidebar's whole refresh.
#[test]
fn an_unreadable_manifest_lists_none() {
    let root = temp_root("broken");
    std::fs::write(root.join("fleet.toml"), "[[runtime]\nname = ").expect("fleet.toml");
    let answer = answer("managed", json!({ "fleetRoot": root })).expect("an answer");
    assert_eq!(answer["runtimes"], json!([]));
    assert!(answer["problem"]
        .as_str()
        .is_some_and(|problem| !problem.is_empty()));
    let _ = std::fs::remove_dir_all(&root);
}

/// The logs as readable event summaries (specification 25 §6.5): a record's `data` object gives the
/// level, the event and the details in the record's order, `ts` left out; a record without `data`
/// is read whole; a line that is no record stays as it is; no logs say so. Catches raw JSON records
/// in the output and details in another order than the runtime wrote them.
#[test]
fn the_logs_are_readable_event_summaries() {
    let stdout = [
        r#"{"ts":1,"data":{"level":"warn","event":"io.retry","zeta":2,"alpha":"x","nested":{"b":1,"a":[1,2]}}}"#,
        r#"{"level":"error","event":"scan.fault","code":"IndexOutOfBounds","ts":"t"}"#,
        r#"{"data":{"value":null,"ok":true}}"#,
        "plain text line  ",
        "",
    ]
    .join("\n");
    let answer = answer(
        "managedLogs",
        json!({ "name": "cell1", "stdout": stdout, "stderr": "ignored" }),
    )
    .expect("an answer");
    assert_eq!(
        answer["text"],
        "[warn] io.retry zeta=2 alpha=x nested={\"b\":1,\"a\":[1,2]}\n\
         [error] scan.fault code=IndexOutOfBounds\n\
         [info] runtime value=null ok=true\n\
         plain text line\n\
         \n"
    );
    let from_stderr = answer_text("", "  boom  ");
    assert_eq!(from_stderr, "boom\n");
    assert_eq!(answer_text("  ", " "), "No logs available for cell1.\n");
}

fn answer_text(stdout: &str, stderr: &str) -> String {
    answer(
        "managedLogs",
        json!({ "name": "cell1", "stdout": stdout, "stderr": stderr }),
    )
    .expect("an answer")["text"]
        .as_str()
        .expect("a text")
        .to_owned()
}

fn stop_ends(stopped_endpoint: &str, stopped_label: &str, attached: Value) -> bool {
    answer(
        "managedStopEndsSession",
        json!({
            "stoppedEndpoint": stopped_endpoint,
            "stoppedLabel": stopped_label,
            "attached": attached,
        }),
    )
    .expect("an answer")["ends"]
        .as_bool()
        .expect("a boolean")
}

/// Specification 25 §5.1: stopping a managed runtime ends the session the extension holds with
/// that runtime, which it knows by the control endpoint (spaces aside) or by the runtime's name,
/// and only an online, connected session. Catches a Stop that leaves a dead session attached, one
/// that ends a session with another runtime, a blank endpoint or name that matches a blank one,
/// and a simulator session ended by a managed Stop.
#[test]
fn a_managed_stop_ends_only_the_session_with_that_runtime() {
    let connected = |endpoint: &str, label: &str| json!({ "mode": "online", "state": "connected", "endpoint": endpoint, "label": label });
    let tcp = "tcp://127.0.0.1:5680";
    assert!(stop_ends(tcp, "", connected(" tcp://127.0.0.1:5680 ", "")));
    assert!(stop_ends(
        "",
        "line-1",
        connected("tcp://10.0.0.2:5680", "line-1")
    ));
    assert!(!stop_ends(
        tcp,
        "line-1",
        connected("tcp://10.0.0.2:5680", "line-2")
    ));
    assert!(!stop_ends("", "", connected("", "")));
    assert!(!stop_ends(
        tcp,
        "",
        json!({ "mode": "simulate", "state": "connected", "endpoint": tcp })
    ));
    assert!(!stop_ends(
        tcp,
        "",
        json!({ "mode": "online", "state": "disconnected", "endpoint": tcp })
    ));
}
