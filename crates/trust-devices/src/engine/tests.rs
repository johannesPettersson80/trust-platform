//! The engine's own rules (design §1.5): the `seq` of a view, the result of a slow action, and the
//! searches that closing a session ends. Every expected value is worked out from the design line
//! that each test names.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use super::{millis, DevicesEngine, SearchCall, SearchOrigin, SearchVerb, WAITING_REFRESH_MS};
use crate::actions::Fleet;
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::gather::Budget;
use crate::request::{
    ActOp, ActRequest, GatherMode, HostFacts, HostSettings, RunMode, ViewOptions, ViewRequest,
};
use crate::test_project::{runtime_toml, TempFolder};
use crate::view::types::{HostEffect, OperationDone, Refresh, Secret, Tone};

/// The `trust-runtime` executable that a managed-start request names; the fake fleet never runs
/// it.
const TRUST_RUNTIME: &str = "/opt/trust-runtime/trust-runtime";

/// How long a test waits for a background thread.
pub(super) const SLACK: Duration = Duration::from_secs(10);

/// How long the slow work of these tests takes. A test that waits for it fails the bound of
/// [`ANSWERED_BEFORE`] by a wide margin, so the bound measures waiting, not the machine's speed.
const SLOW_WORK: Duration = Duration::from_secs(2);

/// The bound of an answer that does not wait for the slow work (design §1.5). It is far below
/// [`SLOW_WORK`], so a loaded build server cannot fail it on its own.
const ANSWERED_BEFORE: Duration = Duration::from_millis(500);

/// A host in online mode whose control endpoint is `endpoint`.
pub(super) fn online(endpoint: &str) -> HostFacts {
    HostFacts {
        settings: HostSettings {
            mode: RunMode::Online,
            control_endpoint: endpoint.to_owned(),
            ..HostSettings::default()
        },
        ..HostFacts::default()
    }
}

/// One view request of `session` with the number `seq`.
fn view_of(session: &str, seq: u64, gather: GatherMode, host: HostFacts) -> ViewRequest {
    ViewRequest {
        session: session.to_owned(),
        seq,
        gather,
        project: None,
        workspace_folders: Vec::new(),
        view: ViewOptions::default(),
        host,
    }
}

/// One search request of `session` with `verb`, and no search ID.
pub(super) fn search_of(session: &str, verb: SearchVerb) -> SearchCall {
    SearchCall {
        session: session.to_owned(),
        verb,
        origin: SearchOrigin::ThisHost,
        search_id: None,
        since: None,
        params: Value::Null,
        targets: None,
        host: HostFacts::default(),
        change: None,
    }
}

/// Waits until the gather of `session` has arrived, as the extension's next polls take it: only a
/// view installs a gather a background thread has finished (design §1.5).
fn settle(engine: &DevicesEngine, session: &str, seq: &mut u64, host: &HostFacts) {
    let until = Instant::now() + SLACK;
    loop {
        *seq += 1;
        // A `none` gather still installs a finished gather; it never asks a target itself.
        let reply = engine.view(view_of(session, *seq, GatherMode::Cached, host.clone()));
        assert!(
            reply.error.is_none(),
            "the panel answered {:?}",
            reply.error
        );
        if !engine.gathering(session) {
            return;
        }
        assert!(Instant::now() < until, "the gather arrives");
        thread::sleep(Duration::from_millis(5));
    }
}

/// Polls the panel of `session` until a view delivers results of slow actions, and returns them.
/// `seq` is the number of the last request; the next request takes the next number.
fn next_results(engine: &DevicesEngine, session: &str, seq: &mut u64) -> Vec<OperationDone> {
    let until = Instant::now() + SLACK;
    loop {
        *seq += 1;
        let reply = engine.view(view_of(
            session,
            *seq,
            GatherMode::Cached,
            HostFacts::default(),
        ));
        assert!(
            reply.error.is_none(),
            "the panel answered {:?}",
            reply.error
        );
        if !reply.operations_done.is_empty() {
            return reply.operations_done;
        }
        assert!(Instant::now() < until, "the result arrives in a later view");
        thread::sleep(Duration::from_millis(5));
    }
}

/// A loopback port that accepts connections and answers none of them, so a probe of it waits
/// for its own timeout and its search is still running.
fn silent_port() -> String {
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

/// A [`Fleet`] whose Start answers without running a process.
struct FakeFleet {
    start: Value,
}

impl Fleet for FakeFleet {
    fn start(&self, _executable: &Path, _fleet_root: &Path, _name: &str) -> Result<Value, String> {
        Ok(self.start.clone())
    }

    fn stop(&self, _executable: &Path, _fleet_root: &Path, _name: &str) -> Result<Value, String> {
        Err("the fake does not stop a runtime".to_owned())
    }

    fn add(
        &self,
        _executable: &Path,
        _fleet_root: &Path,
        _name: &str,
        _template: &str,
    ) -> Result<Value, String> {
        Err("the fake does not scaffold a project".to_owned())
    }
}

/// A [`Fleet`] whose Start is slow work of [`SLOW_WORK`], which records when it finished.
struct SlowFleet {
    finished: Arc<AtomicBool>,
    start: Value,
}

impl Fleet for SlowFleet {
    fn start(&self, _executable: &Path, _fleet_root: &Path, _name: &str) -> Result<Value, String> {
        thread::sleep(SLOW_WORK);
        self.finished.store(true, Ordering::SeqCst);
        Ok(self.start.clone())
    }

    fn stop(&self, _executable: &Path, _fleet_root: &Path, _name: &str) -> Result<Value, String> {
        Err("the fake does not stop a runtime".to_owned())
    }

    fn add(
        &self,
        _executable: &Path,
        _fleet_root: &Path,
        _name: &str,
        _template: &str,
    ) -> Result<Value, String> {
        Err("the fake does not scaffold a project".to_owned())
    }
}

/// The project folder of the managed runtime `cell`, with its token.
fn cell_folder() -> TempFolder {
    let folder = TempFolder::new("engine-cell");
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", "tcp://127.0.0.1:9901", Some("token-1")),
    );
    folder
}

/// The request that starts the managed runtime `cell` of `folder`.
fn start_request(folder: &TempFolder) -> ActRequest {
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: "runtime.managed_start".to_owned(),
            payload: json!({ "runtime": "trust-fleet-v1:cell", "name": "cell" }),
        },
        values: Map::new(),
        project: Some(folder.path.clone()),
        host: HostFacts {
            runtime_executable: Some(PathBuf::from(TRUST_RUNTIME)),
            ..HostFacts::default()
        },
    }
}

/// The answer of a fleet Start that reached `running` at the endpoint of `cell`.
fn running_answer() -> Value {
    json!({
        "status": "running",
        "control_endpoint": "tcp://127.0.0.1:9901",
        "path": "cell",
        "message": "Runtime started.",
    })
}

#[test]
fn older_seq_is_not_gathered_twice() {
    // Design §1.5: "Each poll carries an increasing `seq`. ... Rust checks before projecting
    // whether the session has seen a newer `seq`. If it has, it returns the cache's projection
    // without a second gather." A wrong implementation asks the targets again for the older
    // poll, so every late reply restarts the gather.
    let runtime = FakeRuntime::start(Script::running());
    let engine = DevicesEngine::new();
    let host = online(&runtime.endpoint);
    let mut seq = 0_u64;

    seq += 1;
    let first = engine.view(view_of("dc-test", seq, GatherMode::Now, host.clone()));
    // The first view of a session is the loading shell, and it asks the targets again soon
    // (design §1.5).
    assert_eq!(first.next_refresh_ms, WAITING_REFRESH_MS);
    settle(&engine, "dc-test", &mut seq, &host);
    assert_eq!(
        runtime.count("status"),
        1,
        "one gather asks the runtime once"
    );

    seq += 1;
    let second = engine.view(view_of("dc-test", seq, GatherMode::Now, host.clone()));
    assert_eq!(
        second.next_refresh_ms, WAITING_REFRESH_MS,
        "the view that starts a gather asks again soon"
    );
    settle(&engine, "dc-test", &mut seq, &host);
    assert_eq!(runtime.count("status"), 2);

    let older = engine.view(view_of("dc-test", 1, GatherMode::Now, host));
    assert!(
        !engine.gathering("dc-test"),
        "an older seq starts no gather"
    );
    assert_eq!(
        runtime.count("status"),
        2,
        "an older seq is never gathered twice"
    );
    assert_eq!(
        older.next_refresh_ms,
        millis(Budget::default().period),
        "an older seq is the cache's projection, not the loading shell"
    );
}

#[test]
fn pending_operation_result_arrives_once_in_a_later_view() {
    // Design §1.4.1: "A slow action answers ... `pending` ... Its final `notice` and `effects`
    // arrive exactly once, in the `operations_done` of a later `devicesView` reply" (§1.5). A
    // wrong implementation loses the result, delivers it twice, or delivers it in `act` itself.
    let engine = DevicesEngine::new();
    let folder = cell_folder();
    let fleet = FakeFleet {
        start: running_answer(),
    };

    let reply = engine.act_with(&start_request(&folder), Arc::new(fleet));
    let pending = reply
        .pending
        .clone()
        .expect("a Start answers with a pending operation");
    assert_eq!(reply.refresh, Refresh::Soon);
    assert_eq!(pending.pane, "inspector:trust-fleet-v1:cell");

    let mut seq = 0_u64;
    let delivered = next_results(&engine, "dc-test", &mut seq);
    assert_eq!(delivered.len(), 1, "the result arrives once");
    let done = &delivered[0];
    assert_eq!(
        done.operation, pending.operation,
        "the notice answers the operation that was announced"
    );
    assert_eq!(
        done.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some(pending.pane.as_str())
    );
    assert_eq!(
        done.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("Runtime started.")
    );
    assert_eq!(
        done.effects,
        vec![
            HostEffect::StoreSecret {
                endpoint: "tcp://127.0.0.1:9901".to_owned(),
                token: Secret("token-1".to_owned()),
            },
            HostEffect::Attach {
                endpoint: "tcp://127.0.0.1:9901".to_owned(),
                label: "cell (this computer)".to_owned(),
            },
            HostEffect::SetRunTarget {
                id: "cell".to_owned(),
            },
        ]
    );

    seq += 1;
    let second = engine.view(view_of(
        "dc-test",
        seq,
        GatherMode::Cached,
        HostFacts::default(),
    ));
    assert_eq!(
        second.operations_done,
        Vec::<OperationDone>::new(),
        "each result is delivered exactly once"
    );
}

#[test]
fn a_view_answers_while_a_slow_action_runs() {
    // Design §1.5: "every devices command answers ... from the session", and "background threads
    // hold no ServerState lock". The Start is slow work of two seconds. A wrong implementation runs
    // it while it holds the panel, so the view waits for the Start, and every poll of the panel
    // waits with it.
    let engine = DevicesEngine::new();
    let folder = cell_folder();
    let finished = Arc::new(AtomicBool::new(false));
    let fleet = SlowFleet {
        finished: Arc::clone(&finished),
        start: running_answer(),
    };

    let reply = engine.act_with(&start_request(&folder), Arc::new(fleet));
    let pending = reply
        .pending
        .clone()
        .expect("a Start that is still running answers pending");

    let started = Instant::now();
    let view = engine.view(view_of(
        "dc-test",
        1,
        GatherMode::Cached,
        HostFacts::default(),
    ));
    let waited = started.elapsed();
    assert!(
        waited < ANSWERED_BEFORE,
        "the view answered before the Start's {SLOW_WORK:?} of work: {waited:?}"
    );
    assert!(
        !finished.load(Ordering::SeqCst),
        "the Start is still running when the view answers"
    );
    assert_eq!(
        view.operations_done,
        Vec::<OperationDone>::new(),
        "the Start has not finished"
    );

    let mut seq = 1_u64;
    let delivered = next_results(&engine, "dc-test", &mut seq);
    assert!(
        finished.load(Ordering::SeqCst),
        "the result arrives only after the Start finished"
    );
    assert_eq!(delivered.len(), 1, "the finished Start is delivered once");
    assert_eq!(delivered[0].operation, pending.operation);
}

#[test]
fn close_ends_the_session_and_its_searches() {
    // Design §1.4.1: "`trust-lsp.devicesClose` `{session}`: ends the session and cancels its
    // searches." A wrong implementation ends the session and lets the search run to its
    // deadline.
    let engine = DevicesEngine::new();
    let address = silent_port();
    let mut start = search_of("dc-test", SearchVerb::Start);
    start.params = json!({
        "only": ["modbus_tcp"],
        "targets": { "hosts": [address] },
        "deadline_ms": 30000,
    });
    let started = engine.search(&start);
    let search_id = started.search_id.clone().expect("a started search");

    let mut status = search_of("dc-test", SearchVerb::Status);
    status.search_id = Some(search_id.clone());
    assert_eq!(
        engine.search(&status).status.get("state"),
        Some(&json!("running")),
        "the search runs until it is cancelled"
    );

    let closed = engine.close("dc-test");
    assert_eq!(closed.session, "dc-test");
    assert!(closed.closed, "the session was open");

    assert_eq!(
        engine.search(&status).status.get("state"),
        Some(&json!("cancelled")),
        "closing the session cancels its searches"
    );
}

#[test]
fn a_runtime_search_answers_at_once_when_the_runtime_stays_silent() {
    // Design §1.5: every devices command answers from the session, so a runtime's own timeouts never
    // hold a request. The silent runtime holds each request for its 2-second timeout, which is the
    // slow work here. A wrong implementation forwards the search inside the request, so the answer
    // waits for that timeout.
    let address = silent_port();
    let folder = TempFolder::new("silent-runtime");
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", &format!("tcp://{address}"), Some("token-1")),
    );
    let project = folder.join("cell");
    let engine = DevicesEngine::new();
    let project_view = |seq: u64, gather: GatherMode| ViewRequest {
        session: "dc-test".to_owned(),
        seq,
        gather,
        project: Some(project.clone()),
        workspace_folders: vec![folder.path.clone()],
        view: ViewOptions::default(),
        host: HostFacts::default(),
    };
    let _ = engine.view(project_view(1, GatherMode::Now));
    let until = Instant::now() + SLACK;
    let mut seq = 1_u64;
    // The card that the project's runtime shows, once its gather has ended (design §1.5).
    let card = loop {
        seq += 1;
        let shown = engine
            .view(project_view(seq, GatherMode::Cached))
            .view
            .expect("the panel shows the project");
        let gathered = !engine.gathering("dc-test");
        let first = shown
            .computers
            .iter()
            .flat_map(|computer| &computer.runtimes)
            .next();
        if let Some(card) = first.filter(|_| gathered) {
            break card.id.as_str().to_owned();
        }
        assert!(
            Instant::now() < until,
            "the project's runtime card is shown"
        );
        thread::sleep(Duration::from_millis(5));
    };
    let call = SearchCall {
        session: "dc-test".to_owned(),
        verb: SearchVerb::Start,
        origin: SearchOrigin::Runtime(card),
        search_id: None,
        since: None,
        params: json!({ "only": ["modbus_tcp"], "targets": { "hosts": ["127.0.0.1:502"] } }),
        targets: None,
        host: HostFacts::default(),
        change: None,
    };

    let started = Instant::now();
    let reply = engine.search(&call);
    let waited = started.elapsed();
    assert!(
        waited < ANSWERED_BEFORE,
        "the silent runtime held the request for {waited:?}"
    );
    assert_eq!(
        reply.error.as_ref().map(|error| error.code.as_str()),
        Some("runtime_pending"),
        "the runtime has not answered yet, so the panel asks again"
    );

    let unknown = SearchCall {
        origin: SearchOrigin::Runtime("no-such-runtime".to_owned()),
        ..call
    };
    let refused = engine.search(&unknown);
    assert_eq!(
        refused.error.as_ref().map(|error| error.code.as_str()),
        Some("unknown_runtime"),
        "a runtime the panel never showed is refused by name"
    );
}

/// One view request of the project `project` in the folder `folder`, as the extension sends it: the
/// workspace folder it has open, the project the user chose and the host's settings.
fn project_view(
    seq: u64,
    gather: GatherMode,
    folder: &Path,
    project: &Path,
    host: &HostFacts,
) -> ViewRequest {
    ViewRequest {
        session: "dc-test".to_owned(),
        seq,
        gather,
        project: Some(project.to_path_buf()),
        workspace_folders: vec![folder.to_path_buf()],
        view: ViewOptions::default(),
        host: host.clone(),
    }
}

/// Polls the panel of the project until its gather has ended and its runtime card answers, and
/// returns the card's ID (design §1.5). `seq` is the number of the last request.
pub(super) fn answering_card(
    engine: &DevicesEngine,
    folder: &Path,
    project: &Path,
    host: &HostFacts,
    seq: &mut u64,
) -> String {
    *seq += 1;
    let _ = engine.view(project_view(*seq, GatherMode::Now, folder, project, host));
    let until = Instant::now() + SLACK;
    loop {
        *seq += 1;
        let shown = engine
            .view(project_view(
                *seq,
                GatherMode::Cached,
                folder,
                project,
                host,
            ))
            .view
            .expect("the panel shows the project");
        let card = shown
            .computers
            .iter()
            .flat_map(|computer| &computer.runtimes)
            .find(|item| item.answers)
            .map(|item| item.id.as_str().to_owned());
        let gathered = !engine.gathering("dc-test");
        if let Some(id) = card.filter(|_| gathered) {
            return id;
        }
        assert!(Instant::now() < until, "the runtime's card answers");
        thread::sleep(Duration::from_millis(5));
    }
}

/// Polls the panel of the project until a view delivers results of slow actions, and returns those
/// results with that view's `next_refresh_ms`. `seq` is the number of the last request.
fn late_results(
    engine: &DevicesEngine,
    folder: &Path,
    project: &Path,
    host: &HostFacts,
    seq: &mut u64,
) -> (Vec<OperationDone>, u64) {
    let until = Instant::now() + SLACK;
    loop {
        *seq += 1;
        let polled = engine.view(project_view(
            *seq,
            GatherMode::Cached,
            folder,
            project,
            host,
        ));
        assert!(
            polled.error.is_none(),
            "the panel answered {:?}",
            polled.error
        );
        if !polled.operations_done.is_empty() {
            return (polled.operations_done, polled.next_refresh_ms);
        }
        assert!(Instant::now() < until, "the save's result arrives");
        thread::sleep(Duration::from_millis(5));
    }
}

/// The request that saves a Modbus TCP connection to the runtime `id` of the project `project`.
fn modbus_save(id: &str, project: &Path, host: &HostFacts) -> ActRequest {
    let mut values = Map::new();
    values.insert("address".to_owned(), json!("10.0.0.9:502"));
    values.insert("unit_id".to_owned(), json!(1));
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: "connection.save".to_owned(),
            payload: json!({
                "form": format!("modbus_tcp@{id}"),
                "protocol": "modbus_tcp",
                "runtime": id,
            }),
        },
        values,
        project: Some(project.to_path_buf()),
        host: host.clone(),
    }
}

#[test]
fn a_late_save_closes_its_form() {
    // Design §1.4.1 and specification 25 §5.15: an action that runs past the wait answers `pending`,
    // and its final reply arrives once, in a later view, with the fields of an on-time reply that the
    // panel acts on: the notice, the effects, `close_pane`, `refresh` and `form`. The save is slow
    // work of about 2.4 seconds: the runtime keeps its connection alive meanwhile. A wrong
    // implementation drops `close_pane`, so the form of a save that runs past the wait stays open.
    // It drops the refresh too, so the panel keeps the state from before the save.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer("comm.schema", Reply::Refuse("unsupported_request"))
            .trickled("comm.apply", Duration::from_millis(400), 6),
    );
    let folder = TempFolder::new("late-save");
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", &runtime.endpoint, Some("token-1")),
    );
    let project = folder.join("cell");
    let host = online(&runtime.endpoint);
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let id = answering_card(&engine, &folder.path, &project, &host, &mut seq);
    let statuses = runtime.count("status");

    let started = Instant::now();
    let reply = engine.act(&modbus_save(&id, &project, &host));
    let waited = started.elapsed();
    assert!(
        waited < ANSWERED_BEFORE,
        "the save answered before its slow work: {waited:?}"
    );
    let pending = reply
        .pending
        .clone()
        .expect("a save that runs past the wait answers pending");
    assert_eq!(
        reply.refresh,
        Refresh::Soon,
        "the pending answer asks for a soon refresh"
    );

    let (delivered, refresh_ms) = late_results(&engine, &folder.path, &project, &host, &mut seq);
    assert!(
        started.elapsed() >= SLOW_WORK,
        "the result arrives after the save's slow work: {:?}",
        started.elapsed()
    );
    assert_eq!(delivered.len(), 1, "the save's result arrives once");
    let done = &delivered[0];
    assert_eq!(
        done.operation, pending.operation,
        "the result answers the operation that was announced"
    );
    assert!(
        done.close_pane,
        "the pane of the save closes, as an on-time save closes it"
    );
    assert_eq!(done.refresh, Refresh::Now, "the save asks for a refresh");
    assert_eq!(
        refresh_ms, 0,
        "the view that delivers the result asks the extension to poll at once"
    );
    let pane = format!("form:modbus_tcp@{id}");
    assert_eq!(
        done.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some(pane.as_str()),
        "the notice names the form's pane"
    );
    assert_eq!(
        done.notice.as_ref().map(|notice| notice.tone),
        Some(Tone::Ok),
        "the save was applied"
    );

    // The refresh gathers the new state, which asks the runtime for its status again.
    let until = Instant::now() + SLACK;
    while runtime.count("status") <= statuses {
        assert!(
            Instant::now() < until,
            "the refresh asks the runtime for its status again"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        runtime.count("comm.apply"),
        1,
        "the save reached the runtime once"
    );
}

/// A start of a search at the runtime with item ID `origin`, with the parameters `params`.
pub(super) fn start_at(origin: &str, params: Value) -> SearchCall {
    SearchCall {
        session: "dc-test".to_owned(),
        verb: SearchVerb::Start,
        origin: SearchOrigin::Runtime(origin.to_owned()),
        search_id: None,
        since: None,
        params,
        targets: None,
        host: HostFacts::default(),
        change: None,
    }
}

/// The answer of a runtime search, asked again while the runtime has not answered in time: the
/// panel waits for a runtime for a short while, then says `runtime_pending`, and the panel asks
/// again.
pub(super) fn answered(engine: &DevicesEngine, call: &SearchCall) -> super::SearchReply {
    let until = Instant::now() + SLACK;
    loop {
        let reply = engine.search(call);
        let pending = reply
            .error
            .as_ref()
            .is_some_and(|error| error.code == "runtime_pending");
        if !pending {
            return reply;
        }
        assert!(Instant::now() < until, "the runtime answers the search");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn this_host_search_runs_in_process() {
    // Design §1.4.1: "Origin `this_host` runs the engine in the language-server process", and
    // specification 11 ("One search"): the search of this computer is started in this process, so
    // the first search of a process is `s-1`, and its status names the origin `this_host`. A
    // wrong implementation forwards the search to a control endpoint, which this test never
    // gives, or names another origin.
    let engine = DevicesEngine::new();
    let mut start = search_of("dc-here", SearchVerb::Start);
    start.params = json!({ "only": ["gpio"], "deadline_ms": 1000 });
    let started = engine.search(&start);
    assert!(
        started.error.is_none(),
        "the search starts on this computer"
    );
    assert_eq!(
        started.search_id.as_deref(),
        Some("s-1"),
        "the first search of the process is s-1"
    );

    let mut status = search_of("dc-here", SearchVerb::Status);
    status.search_id = Some("s-1".to_owned());
    let reply = engine.search(&status);
    assert_eq!(reply.status.get("origin"), Some(&json!("this_host")));
    assert_eq!(
        reply.status.get("state"),
        Some(&json!("done")),
        "a source that did not run ends the search at once"
    );
    assert_eq!(
        reply.status.get("sources"),
        Some(&json!([{
            "id": "gpio",
            "state": "skipped",
            "reason": "GPIO is searched inside a runtime, on the computer with the GPIO lines."
        }])),
        "GPIO runs only inside a runtime (specification 11, Sources)"
    );
    assert!(
        reply.discover.is_some(),
        "a status carries the Discover view"
    );
}

#[test]
fn runtime_origin_forwards_the_same_body() {
    // Design §1.4.1: origin `{ "runtime": <item id> }` "forwards the same body to that runtime over
    // the control client", and the runtime runs it as its own search (specification 11, "One
    // search": origin `runtime`). A wrong implementation drops a member of the body, or forwards
    // another origin.
    let runtime = FakeRuntime::start(Script::running().answer(
        "comm.search.start",
        Reply::Result(json!({ "search_id": "s-7" })),
    ));
    let folder = TempFolder::new("forward-search");
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", &runtime.endpoint, Some("token-1")),
    );
    let project = folder.join("cell");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = answering_card(
        &engine,
        &folder.path,
        &project,
        &online(&runtime.endpoint),
        &mut seq,
    );
    let body = json!({
        "only": ["modbus_tcp"],
        "targets": { "hosts": ["127.0.0.1:502"] },
        "deadline_ms": 5000,
    });
    let reply = answered(&engine, &start_at(&card, body));
    assert_eq!(
        reply.search_id.as_deref(),
        Some("s-7"),
        "the runtime's search ID is the answer"
    );

    let forwarded: Vec<Value> = runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "comm.search.start")
        .collect();
    assert!(!forwarded.is_empty(), "the runtime received the start");
    for request in forwarded {
        assert_eq!(
            request["params"],
            json!({
                "only": ["modbus_tcp"],
                "targets": { "hosts": ["127.0.0.1:502"] },
                "deadline_ms": 5000,
                "origin": "runtime"
            }),
            "the same body, with the origin the runtime runs it at"
        );
    }
}

#[test]
fn older_runtime_skips_every_source_with_an_update_reason() {
    // Design §1.4.1: "A runtime that answers `unsupported request` reports every source `skipped`
    // with 'This runtime's truST version cannot search; update it to {version} or later.'"
    // (specification 34 TARGET-05). The version is this tool's. A wrong implementation reports the
    // refusal as the error of the request, skips the sources with another reason, or leaves the
    // Discover view out.
    let runtime = FakeRuntime::start(Script::running());
    let folder = TempFolder::new("older-search");
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", &runtime.endpoint, Some("token-1")),
    );
    let project = folder.join("cell");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = answering_card(
        &engine,
        &folder.path,
        &project,
        &online(&runtime.endpoint),
        &mut seq,
    );
    let reply = answered(&engine, &start_at(&card, json!({})));
    assert!(
        reply.error.is_none(),
        "an older runtime is a status, not an error"
    );

    let reason = format!(
        "This runtime's truST version cannot search; update it to {} or later.",
        env!("CARGO_PKG_VERSION")
    );
    let sources = reply
        .status
        .get("sources")
        .and_then(Value::as_array)
        .expect("the status lists the sources");
    assert_eq!(sources.len(), 10, "every source of the table is reported");
    for source in sources {
        assert_eq!(source["state"], json!("skipped"));
        assert_eq!(source["reason"], json!(reason));
    }
    let discover = reply
        .discover
        .expect("the status carries the Discover view");
    assert_eq!(discover.sources.len(), 10);
    assert!(
        discover
            .sources
            .iter()
            .all(|row| row.word == "skipped" && row.reason.as_deref() == Some(reason.as_str())),
        "the pane shows every source skipped with the update it needs"
    );
}

#[test]
fn a_polled_status_projects_every_device_of_the_search() {
    // Specification 11 ("One search"): with `since`, the status lists only the devices that changed
    // after its cursor, and the caller keeps the rest. The pane keeps one card per device of the
    // whole search (specification 25 §2), so the Discover view of a polled status is projected from
    // the whole status, which the runtime answers without `since`. A wrong implementation projects
    // the changed devices alone, so the pane drops the others; it shows in the requests the runtime
    // receives, because the answer is the same whether or not `since` is sent.
    let status_body = json!({
        "schema_version": 1,
        "search_id": "s-7",
        "origin": "runtime",
        "state": "done",
        "deadline_ms": 10000,
        "elapsed_ms": 900,
        "cursor": 5,
        "sources": [{ "id": "modbus_tcp", "state": "found", "count": 1 }],
        "devices": [{
            "id": "dev:ip:192.168.1.20",
            "addresses": ["192.168.1.20"],
            "last_seen_ms": 1,
            "finds": [{
                "protocol": "modbus_tcp",
                "source": "tcp_connect",
                "confidence": "port_reachable",
                "simulated": false,
                "identity": { "kind": "modbus", "unit_id": 1 },
                "params": { "host": "192.168.1.20", "port": 502, "unit_id": 1 },
                "warnings": [],
                "last_seen_ms": 1
            }]
        }],
        "warnings": []
    });
    let runtime = FakeRuntime::start(
        Script::running().answer("comm.search.status", Reply::Result(status_body)),
    );
    let folder = TempFolder::new("polled-search");
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", &runtime.endpoint, Some("token-1")),
    );
    let project = folder.join("cell");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = answering_card(
        &engine,
        &folder.path,
        &project,
        &online(&runtime.endpoint),
        &mut seq,
    );
    let mut poll = search_of("dc-test", SearchVerb::Status);
    poll.origin = SearchOrigin::Runtime(card);
    poll.search_id = Some("s-7".to_owned());
    poll.since = Some(5);
    let reply = answered(&engine, &poll);
    assert!(reply.error.is_none(), "the poll is answered");

    let cards = reply
        .discover
        .expect("the poll carries the Discover view")
        .cards;
    assert_eq!(cards.len(), 1, "the one device of the search is a card");
    let statuses: Vec<Value> = runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "comm.search.status")
        .map(|request| request["params"].clone())
        .collect();
    assert!(
        statuses.contains(&json!({ "search_id": "s-7", "since": 5 })),
        "the poll is forwarded with its cursor"
    );
    assert!(
        statuses.contains(&json!({ "search_id": "s-7" })),
        "the Discover view is projected from the whole status"
    );
}
