//! The devices commands of the language server (design §1.4.1 and §1.5; specification 25 §5.15):
//! the names the extension must never share, the names the server advertises, the canvas's two
//! commands reaching their rules, the answers that do not wait for a runtime that never answers,
//! and the request slots that slow work leaves free. The timing bounds are 500 ms against two
//! seconds of slow work, so they measure waiting and not the build server's speed.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tower_lsp::lsp_types::{ExecuteCommandParams, InitializeParams};
#[cfg(unix)]
use tower_lsp::lsp_types::{
    HoverParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
};
use tower_lsp::LanguageServer;

use super::{
    DEVICES_ACT_COMMAND, DEVICES_BROWSE_COMMAND, DEVICES_CANVAS_ACT_COMMAND, DEVICES_CLOSE_COMMAND,
    DEVICES_COMMANDS, DEVICES_SEARCH_COMMAND, DEVICES_TOPOLOGY_COMMAND, DEVICES_VIEW_COMMAND,
};
use crate::state::ServerState;
use crate::test_support::test_client;

/// The bound of an answer while the work that a command starts is slow (design §1.5). The slow work
/// of these tests takes two seconds, so a command that waits for it fails this bound by a wide
/// margin, while a loaded build server does not. The spec's 25 ms wait is not asserted here,
/// because no build server holds a bound that tight under load.
const ANSWERED_BEFORE: Duration = Duration::from_millis(500);

/// The session of the panel the timing tests drive.
const SESSION: &str = "dc-measure";

#[test]
fn advertised_commands_never_share_a_name_with_an_extension_command() {
    // Design §1.4.1: "The names follow the lesson of round 5b: one camel-case segment after
    // `trust-lsp.`, so they can never collide with an extension command such as
    // `trust-lsp.networkCanvas.open`", and the language client registers every command the server
    // advertises, so a shared name stops the client from starting. A wrong implementation names a
    // devices command that the extension also defines.
    let extension = repository().join("editors/vscode");
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(extension.join("package.json")).expect("the extension's manifest"),
    )
    .expect("the extension's manifest is JSON");
    let contributed = manifest["contributes"]["commands"]
        .as_array()
        .expect("the extension contributes commands")
        .iter()
        .filter_map(|command| command["command"].as_str());
    let activating = manifest["activationEvents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|event| event.as_str()?.strip_prefix("onCommand:"));
    let registered = registered_commands(&extension.join("src"));
    let extension_commands: Vec<&str> = contributed
        .chain(activating)
        .chain(registered.iter().map(String::as_str))
        .collect();

    for command in DEVICES_COMMANDS {
        assert!(
            !extension_commands.contains(&command),
            "{command} is also an extension command: {extension_commands:?}"
        );
    }
}

#[tokio::test]
async fn the_devices_commands_are_advertised() {
    // Design §1.4.1: the five commands are "advertised in `crates/trust-lsp/src/main.rs:135-142`",
    // and so are the canvas's two (specification 25 §5.15).
    // A wrong implementation answers `workspace/executeCommand` but never advertises the names, so
    // the extension's client refuses to send them.
    let server = crate::StLanguageServer::new(test_client());
    let result = server
        .initialize(InitializeParams::default())
        .await
        .expect("initialize");
    let advertised = result
        .capabilities
        .execute_command_provider
        .expect("an executeCommand provider")
        .commands;
    for command in DEVICES_COMMANDS {
        assert!(
            advertised.iter().any(|name| name.as_str() == command),
            "{command} is advertised: {advertised:?}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn devices_commands_answer_before_a_silent_runtime_gives_up() {
    // Design §1.5: "the language server serves at most four requests at a time ... So every devices
    // command answers from the session. It hands slow work to background threads". The silent
    // runtime holds each of its requests for two seconds, in the background only. A wrong
    // implementation gathers the silent runtime, or runs the browse, inside the request, so the
    // answer waits for those two seconds.
    let address = silent_address();
    let endpoint = format!("tcp://{address}");
    let state = ServerState::new();
    let commands = [
        (DEVICES_VIEW_COMMAND, view_argument(SESSION, 1, &endpoint)),
        (
            DEVICES_ACT_COMMAND,
            json!({
                "session": SESSION,
                "op": { "action": "runtime.connect",
                        "payload": { "endpoint": "tcp://127.0.0.1:9901", "label": "cell" } },
            }),
        ),
        (
            DEVICES_BROWSE_COMMAND,
            json!({
                "session": SESSION, "seq": 1,
                "source": { "connection": "trust-fleet-v1:unknown", "connection_name": "line1" },
            }),
        ),
        (
            DEVICES_SEARCH_COMMAND,
            json!({
                "session": SESSION, "verb": "start", "origin": "this_host",
                "params": { "only": ["modbus_tcp"], "targets": { "hosts": [address] } },
            }),
        ),
        (DEVICES_CLOSE_COMMAND, json!({ "session": SESSION })),
    ];

    for (command, argument) in commands {
        let started = Instant::now();
        let reply = super::execute(&state, &command_params(command, argument))
            .await
            .expect("a devices reply");
        let waited = started.elapsed();
        // A reply error carries a stable code (design §1.4.1); a browse pane's own error box, which
        // it answers at once for a connection its session never showed, carries none.
        let refused = reply
            .get("error")
            .and_then(|error| error.get("code"))
            .is_some();
        assert!(!refused, "{command} answered an error reply: {reply}");
        assert!(
            waited < ANSWERED_BEFORE,
            "{command} answered in {waited:?}, before the silent runtime's two seconds"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_canvas_commands_answer_with_the_canvas_rules() {
    // Specification 25 §5.15: `trust-lsp.devicesTopology` answers the fleet view of the answers the
    // canvas gathered, and `trust-lsp.devicesCanvasAct` performs one action. A configured peer that
    // did not answer is a computer headed by its label whose runtime keeps the peer's endpoint
    // (§5.6); the typed ADS port ` 301 ` is port 301 (§5.4); an action the canvas does not have is
    // refused with the stable code `unknown_canvas_action`. A wrong implementation leaves a command
    // unrouted (no reply), answers no fleet view, or answers an unknown action as if it were done.
    let state = ServerState::new();
    let topology = super::execute(
        &state,
        &command_params(
            DEVICES_TOPOLOGY_COMMAND,
            json!({
                "peers": [{ "endpoint": "tcp://10.0.0.9:5510", "label": "cell1",
                            "reach": "unreachable" }],
                "run_mode": "online",
            }),
        ),
    )
    .await
    .expect("a topology reply");
    // Nothing reports the project's runtime, so the simulator stands for it first on this computer
    // (§5.6) and the peer's computer follows.
    assert_eq!(topology["fleet"]["hosts"][0]["hostname"], "This computer");
    assert_eq!(topology["fleet"]["hosts"][1]["hostname"], "cell1");
    assert_eq!(
        topology["fleet"]["hosts"][1]["runtimes"][0]["controlEndpoint"],
        "tcp://10.0.0.9:5510"
    );

    let port = super::execute(
        &state,
        &command_params(
            DEVICES_CANVAS_ACT_COMMAND,
            json!({ "op": "ads_port", "args": { "text": " 301 " } }),
        ),
    )
    .await
    .expect("an action reply");
    assert_eq!(port["result"], json!({ "port": 301 }));

    let unknown = super::execute(
        &state,
        &command_params(DEVICES_CANVAS_ACT_COMMAND, json!({ "op": "format_disk" })),
    )
    .await
    .expect("an action reply");
    assert_eq!(unknown["error"]["code"], "unknown_canvas_action");
    assert!(
        unknown.get("applied").is_none(),
        "nothing was done: {unknown}"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn slow_work_never_holds_a_server_slot() {
    // Design §1.5 and §4.3 B9: with four views, a browse and a managed Start outstanding together,
    // `textDocument/hover` still answers at once, and a slow action answers `pending` (design
    // §1.4.1). The Start is slow work of two seconds: its script sleeps that long. A wrong
    // implementation runs the Start inside its request, which then holds a request slot for the two
    // seconds, or gathers inside the views. Every bound is 500 ms, far below the two seconds.
    //
    // One session holds the four views and the browse, as one open panel does; the managed Start
    // runs on a session of its own.
    let address = silent_address();
    let endpoint = format!("tcp://{address}");
    let folder = temp_folder("devices-slots");
    let executable = slow_executable(folder.path());
    let project = folder.path().to_path_buf();
    let state = Arc::new(ServerState::new());

    let started = Instant::now();
    let act = tokio::spawn({
        let state = Arc::clone(&state);
        async move {
            let began = Instant::now();
            let reply = super::execute(
                &state,
                &command_params(
                    DEVICES_ACT_COMMAND,
                    json!({
                        "session": "dc-slow",
                        "op": { "action": "runtime.managed_start",
                                "payload": { "runtime": "trust-fleet-v1:cell", "name": "cell" } },
                        "project": project,
                        "host": { "runtime_executable": executable },
                    }),
                ),
            )
            .await
            .expect("an act reply");
            (began.elapsed(), reply)
        }
    });
    let views: Vec<_> = (1..=4)
        .map(|seq| {
            let state = Arc::clone(&state);
            let endpoint = endpoint.clone();
            tokio::spawn(async move {
                let began = Instant::now();
                let reply = super::execute(
                    &state,
                    &command_params(DEVICES_VIEW_COMMAND, view_argument(SESSION, seq, &endpoint)),
                )
                .await
                .expect("a view reply");
                assert!(reply.get("error").is_none(), "a view answered {reply}");
                began.elapsed()
            })
        })
        .collect();
    let browse = tokio::spawn({
        let state = Arc::clone(&state);
        async move {
            let began = Instant::now();
            let _ = super::execute(
                &state,
                &command_params(
                    DEVICES_BROWSE_COMMAND,
                    json!({
                        "session": SESSION, "seq": 1,
                        "source": { "connection": "trust-fleet-v1:unknown",
                                    "connection_name": "line1" },
                    }),
                ),
            )
            .await
            .expect("a browse reply");
            began.elapsed()
        }
    });

    // Let every request be under way before the hover of the editor is measured.
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    let uri = Url::parse("file:///devices-panel.st").expect("a document URI");
    state.open_document(uri.clone(), 1, "PROGRAM Main\nEND_PROGRAM\n".to_owned());
    let hovered = Instant::now();
    let _ = crate::handlers::hover(
        &state,
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 0),
            },
            work_done_progress_params: Default::default(),
        },
    );
    let hover_waited = hovered.elapsed();
    assert!(
        hover_waited < ANSWERED_BEFORE,
        "the editor's hover answered in {hover_waited:?}"
    );

    for view in views {
        let waited = view.await.expect("a view task");
        assert!(
            waited < ANSWERED_BEFORE,
            "a view answered in {waited:?}, before the Start's two seconds"
        );
    }
    let browsed = browse.await.expect("a browse task");
    assert!(
        browsed < ANSWERED_BEFORE,
        "the browse answered in {browsed:?}, before the Start's two seconds"
    );
    let devices_waited = started.elapsed();
    assert!(
        devices_waited < ANSWERED_BEFORE,
        "four views and a browse never hold the request slots: {devices_waited:?}"
    );

    // The managed Start answers `pending` before its two seconds of work (design §1.4.1). Its
    // result arrives later, once, in a view of its panel, after the work finished.
    let (acted, reply) = act.await.expect("an act task");
    assert!(
        acted < ANSWERED_BEFORE,
        "the managed Start answered pending before its two seconds of work: {acted:?}"
    );
    let operation = reply["pending"]["operation"]
        .as_str()
        .expect("the managed Start answers pending")
        .to_owned();
    let done = next_result(&state, "dc-slow", &operation).await;
    assert!(
        started.elapsed() >= Duration::from_secs(2),
        "the result arrives after the Start's two seconds of work: {:?}",
        started.elapsed()
    );
    assert_eq!(
        done["notice"]["pane"].as_str(),
        Some("inspector:trust-fleet-v1:cell"),
        "the result names the pane that started the Start"
    );
}

/// Polls the panel of `session` until a view delivers the result of `operation`, and answers that
/// result. These views ask no runtime (`gather: none`).
#[cfg(unix)]
async fn next_result(state: &ServerState, session: &str, operation: &str) -> Value {
    let until = Instant::now() + Duration::from_secs(30);
    let mut seq = 1_u64;
    loop {
        let argument = json!({ "session": session, "seq": seq, "gather": "none" });
        let reply = super::execute(state, &command_params(DEVICES_VIEW_COMMAND, argument))
            .await
            .expect("a view reply");
        let found = reply["operations_done"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|item| item["operation"].as_str() == Some(operation))
            .cloned();
        if let Some(done) = found {
            return done;
        }
        assert!(Instant::now() < until, "the result of {operation} arrives");
        seq += 1;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// One `trust-lsp.devicesView` argument of `session`.
fn view_argument(session: &str, seq: u64, endpoint: &str) -> Value {
    json!({
        "session": session,
        "seq": seq,
        "gather": "auto",
        "host": {
            "settings": {
                "mode": "online",
                "control_endpoint": endpoint,
                "use_control_endpoint": true,
            },
        },
    })
}

/// The `workspace/executeCommand` parameters of one devices command.
fn command_params(command: &str, argument: Value) -> ExecuteCommandParams {
    ExecuteCommandParams {
        command: command.to_owned(),
        arguments: vec![argument],
        work_done_progress_params: Default::default(),
    }
}

/// The repository root, from this crate's own folder.
fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The command names that the extension registers itself with `registerCommand("…")` under `root`.
fn registered_commands(root: &Path) -> Vec<String> {
    let mut commands = Vec::new();
    collect_registrations(root, &mut commands);
    commands
}

/// Collects the `registerCommand` literals of every TypeScript file under `folder`.
fn collect_registrations(folder: &Path, commands: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_registrations(&path, commands);
            continue;
        }
        if !matches!(
            path.extension().and_then(std::ffi::OsStr::to_str),
            Some("ts" | "tsx")
        ) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        commands.extend(command_literals(&text));
    }
}

/// The command name of every `registerCommand("…")` literal in `text`. A command that a constant
/// names is contributed in `package.json`, which the caller reads too.
fn command_literals(text: &str) -> Vec<String> {
    const CALL: &str = "registerCommand(";
    let mut commands = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(CALL) {
        rest = &rest[at + CALL.len()..];
        let Some(quoted) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = quoted.find('"') {
            commands.push(quoted[..end].to_owned());
        }
    }
    commands
}

/// A `host:port` address on the loopback that accepts every connection and never answers, so a
/// probe of it waits for its own timeout and its search is still running.
fn silent_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let address = listener
        .local_addr()
        .expect("the listener's address")
        .to_string();
    thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => held.push(stream),
                Err(_) => break,
            }
        }
    });
    address
}

/// A folder of its own under the system's temporary folder, removed when it is dropped.
#[cfg(unix)]
struct TempFolder(PathBuf);

#[cfg(unix)]
impl TempFolder {
    /// The folder's path.
    fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(unix)]
impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A folder for one test.
#[cfg(unix)]
fn temp_folder(label: &str) -> TempFolder {
    let path = std::env::temp_dir().join(format!("trust-lsp-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&path).expect("a test folder");
    TempFolder(path)
}

/// An executable that waits, so the action that runs it is still outstanding while the other
/// requests answer.
#[cfg(unix)]
fn slow_executable(folder: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = folder.join("slow-trust-runtime");
    std::fs::write(&path, "#!/bin/sh\nsleep 2\n").expect("write the script");
    let mut permissions = std::fs::metadata(&path)
        .expect("the script's metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("make the script executable");
    path
}
