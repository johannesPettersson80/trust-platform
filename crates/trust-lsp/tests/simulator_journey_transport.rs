//! The HMI agent's simulator journey through the language server's own transport (specification 43,
//! AT-08): the trust-lsp binary driven over stdio, as the extension's language client drives it,
//! against a simulator HMI served here on 127.0.0.1.
//!
//! These prove what only the real server shows: a journey answers its request at once and reports
//! its end as `trust-lsp/simulatorJourneyEnded`; journeys waiting on a hung simulator never keep the
//! server's other requests waiting (it serves four at a time); and with tower-lsp's, tungstenite's
//! and the language server's own logs at trace level, no log line carries the named session's cookie
//! or the runtime's access token. Each journey exercises a draft that the server composed from a
//! project page, and the simulator serves that page.

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tiny_http::{Header, Method, Response, Server as HttpServer, StatusCode};
use tower_lsp::lsp_types::Url;
use tungstenite::protocol::Role;
use tungstenite::{Message, WebSocket};

const WAIT: Duration = Duration::from_secs(30);

/// The trust-lsp binary as a language server on stdio.
struct LanguageServer {
    child: Child,
    stdin: ChildStdin,
    incoming: Receiver<Value>,
    kept: VecDeque<Value>,
    next_id: i64,
    /// What the server wrote to stderr so far, its log.
    log: Arc<Mutex<String>>,
    stderr: Option<JoinHandle<()>>,
}

impl LanguageServer {
    /// Starts the server with `RUST_LOG` set to `log` and initializes it.
    fn start(log: &str) -> Self {
        let mut child = Command::new(
            std::env::var_os("CARGO_BIN_EXE_trust-lsp")
                .expect("Cargo must provide trust-lsp binary while executing integration tests"),
        )
        .env("RUST_LOG", log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the trust-lsp binary");
        let stdout = child.stdout.take().expect("its stdout");
        let (sender, incoming) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        let stderr = child.stderr.take().expect("its stderr");
        let log = Arc::new(Mutex::new(String::new()));
        let written = Arc::clone(&log);
        let stderr = thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                let mut log = written
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                log.push_str(&line);
                log.push('\n');
            }
        });
        let stdin = child.stdin.take().expect("its stdin");
        let mut server = Self {
            child,
            stdin,
            incoming,
            kept: VecDeque::new(),
            next_id: 1,
            log,
            stderr: Some(stderr),
        };
        let params = json!({ "processId": null, "rootUri": null, "capabilities": {} });
        server.request("initialize", params, WAIT);
        server.send(&json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }));
        server
    }

    fn send(&mut self, message: &Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).expect("a write");
        self.stdin.flush().expect("a flush");
    }

    /// Sends a request; its ID.
    fn ask(&mut self, method: &str, params: Value) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        id
    }

    /// The next message from the server within `timeout`, answering its own requests: no settings
    /// for `workspace/configuration`, `null` for any other.
    fn next(&mut self, timeout: Duration) -> Option<Value> {
        let message = self.incoming.recv_timeout(timeout).ok()?;
        if let Some(method) = message.get("method").and_then(Value::as_str)
            && let Some(id) = message.get("id")
        {
            let result = match method {
                "workspace/configuration" => {
                    let items = message["params"]["items"].as_array().map_or(0, Vec::len);
                    Value::Array(vec![Value::Null; items])
                }
                _ => Value::Null,
            };
            let reply = json!({ "jsonrpc": "2.0", "id": id, "result": result });
            self.send(&reply);
        }
        Some(message)
    }

    /// The response to the request `id`, within `timeout`; other messages are kept.
    fn response(&mut self, id: i64, timeout: Duration) -> Value {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = self.next(left).expect("the response in time");
            if message.get("id") == Some(&json!(id)) && message.get("method").is_none() {
                return message;
            }
            self.kept.push_back(message);
        }
    }

    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Value {
        let id = self.ask(method, params);
        self.response(id, timeout)
    }

    /// The result of the command `name` on `argument`.
    fn command(&mut self, name: &str, argument: Value) -> Value {
        let params = json!({ "command": name, "arguments": [argument] });
        self.request("workspace/executeCommand", params, WAIT)["result"].clone()
    }

    /// The next notification `method`, within `timeout`.
    fn notification(&mut self, method: &str, timeout: Duration) -> Value {
        if let Some(index) = self.kept.iter().position(|kept| kept["method"] == method) {
            return self.kept.remove(index).expect("the kept notification");
        }
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = self.next(left).expect("the notification in time");
            if message["method"] == method {
                return message;
            }
            self.kept.push_back(message);
        }
    }

    /// Whether the server's log holds `text` within `timeout`.
    fn logged(&self, text: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let log = self
                .log
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if log.contains(text) {
                return true;
            }
            drop(log);
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Stops the server: what it wrote to stderr.
    fn stop(mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(stderr) = self.stderr.take() {
            let _ = stderr.join();
        }
        let log = self
            .log
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        log.clone()
    }
}

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

/// The settings page's file.
const SETTINGS_FILE: &str = "hmi/pages/settings.toml";
/// A page with a button without feedback, whose command the runtime does not follow.
const SETTINGS: &str = "format = 1\ntitle = \"Settings\"\n\n[[object]]\nid = \"stop\"\ntype = \"button\"\nat = [32, 160]\nsize = [120, 40]\naction = \"reset\"\n\n[object.bind]\ntarget = { path = \"Main.Run\", direction = \"write\" }\n";
/// The workspace the journeys' drafts belong to.
const WORKSPACE: &str = "workspace-transport";

/// A project folder under the system temporary folder, with a compiling program and the settings
/// page; removed when the test ends.
struct Project(PathBuf);

impl Project {
    fn new(label: &str) -> Self {
        let folder = trust_test_support::unique_temp_path(&format!(
            "trust-lsp-journey-transport-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        for (file, text) in [
            (
                "src/main.st",
                "PROGRAM Main\nVAR Run : BOOL; END_VAR\nEND_PROGRAM\n",
            ),
            (
                "hmi/screen.toml",
                "format = 1\nsize = [1280, 800]\ntheme = \"dark\"\nsymbols = \"flat\"\n",
            ),
            (SETTINGS_FILE, SETTINGS),
        ] {
            let path = folder.join(file);
            fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
            fs::write(path, text).expect("the file");
        }
        Self(folder)
    }

    /// The project folder's address, the workspace of its simulator sessions.
    fn root_uri(&self) -> String {
        Url::from_directory_path(&self.0)
            .expect("a folder address")
            .to_string()
    }

    /// The fields the authoring commands share: the workspace, the project folder and its draft
    /// storage.
    fn fields(&self) -> Value {
        json!({
            "workspaceId": WORKSPACE,
            "root_uri": self.root_uri(),
            "storage": self.0.join("private").display().to_string(),
        })
    }

    /// Has `server` compose a draft that copies the settings page: the draft's ID, at revision 1,
    /// and the page's model as the simulator serves it once the page is deployed.
    fn settings_draft(&self, server: &mut LanguageServer) -> (String, Value) {
        let mut argument = self.fields();
        argument["request"] = json!({
            "contractVersion": 1,
            "workspaceId": WORKSPACE,
            "input": { "key": "transport", "file": SETTINGS_FILE, "sourceFile": SETTINGS_FILE },
        });
        let composed = server.command("trust-lsp.hmiAgent.compose", argument);
        let draft = composed["data"]["draftId"]
            .as_str()
            .unwrap_or_else(|| panic!("a draft: {composed}"))
            .to_owned();
        let parse = json!({ "kind": "page", "text": SETTINGS, "file": SETTINGS_FILE });
        let parsed = server.command("trust-lsp.hmiFileParse", parse);
        assert_eq!(parsed["ok"], true, "{parsed}");
        (draft, parsed["model"].clone())
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A simulator HMI on 127.0.0.1 serving `model` as the page `settings`, its readback stream and its
/// commands, answered `applied`, as the runtime serves them. Its web address.
fn simulator(model: Value) -> String {
    let server = HttpServer::http("127.0.0.1:0").expect("a simulator HMI");
    let address = server.server_addr().to_ip().expect("an IP listener");
    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let model = model.clone();
            thread::spawn(move || {
                let url = request.url().to_owned();
                let json = |status: u16, body: Value| {
                    let kind =
                        Header::from_bytes("Content-Type", "application/json").expect("a header");
                    Response::from_string(body.to_string())
                        .with_status_code(StatusCode(status))
                        .with_header(kind)
                };
                if url.starts_with("/api/hmi/page") {
                    let body =
                        json!({ "page": { "name": "settings", "model": model }, "bundle": "b1" });
                    let _ = request.respond(json(200, body));
                } else if url == "/api/hmi/command" && *request.method() == Method::Post {
                    let mut text = String::new();
                    let _ = request.as_reader().read_to_string(&mut text);
                    let command: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                    let answer = json!({ "id": command["id"], "outcome": "applied" });
                    let _ = request.respond(json(200, answer));
                } else if url.starts_with("/ws/hmi") {
                    let key = request
                        .headers()
                        .iter()
                        .find(|header| header.field.equiv("Sec-WebSocket-Key"))
                        .map(|header| header.value.as_str().to_owned())
                        .unwrap_or_default();
                    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
                    let header =
                        Header::from_bytes("Sec-WebSocket-Accept", accept).expect("a header");
                    let socket = request.upgrade(
                        "websocket",
                        Response::empty(StatusCode(101)).with_header(header),
                    );
                    let mut socket = WebSocket::from_raw_socket(socket, Role::Server, None);
                    let state = json!({ "simulation": { "enabled": true, "time_scale": 1.0 } });
                    if socket.send(Message::text(state.to_string())).is_ok() {
                        while socket.read().is_ok() {}
                    }
                } else {
                    let _ = request.respond(json(404, json!({ "error": "not found" })));
                }
            });
        }
    });
    format!("http://{address}")
}

/// Opens a session of `project`'s workspace for the simulator at `url` with `token`: the target and
/// the session's ID.
fn open(server: &mut LanguageServer, project: &Project, url: &str, token: &str) -> (Value, String) {
    let target = json!({ "simulator": true, "url": url, "endpoint": "tcp://127.0.0.1:9100",
                         "token": token });
    let argument = json!({ "workspace": project.root_uri(), "name": "Fixture", "target": target });
    let opened = server.command("trust-lsp.hmiAgent.openSimulatorSession", argument);
    let id = opened["data"]["sessionId"]
        .as_str()
        .unwrap_or_else(|| panic!("a session: {opened}"))
        .to_owned();
    (target, id)
}

/// The journey `journey-1` of `steps` on the session `id` of `project`, with `cookie`, on revision 1
/// of `draft`.
fn journey(
    project: &Project,
    draft: &str,
    id: &str,
    cookie: &str,
    target: &Value,
    steps: Value,
) -> Value {
    let mut argument = project.fields();
    for (field, value) in [
        ("sessionId", json!(id)),
        ("journeyId", json!("journey-1")),
        ("cookie", json!(cookie)),
        ("target", target.clone()),
        ("steps", steps),
        ("draftId", json!(draft)),
        ("revision", json!("1")),
    ] {
        argument[field] = value;
    }
    argument
}

/// Catches: a journey that answers its request only when it ends, and a journey whose answer never
/// reaches the client as `trust-lsp/simulatorJourneyEnded`.
#[test]
fn a_journey_answers_at_once_and_its_end_arrives_as_a_notification() {
    let project = Project::new("at-once");
    let mut server = LanguageServer::start("warn");
    let (draft, model) = project.settings_draft(&mut server);
    let url = simulator(model);
    let (target, id) = open(&mut server, &project, &url, "access-token-7");
    let steps = json!([{ "page": "settings", "object": "stop", "action": "reset" }]);
    let cookie = "trust_hmi_session=4f1c0ffee";
    let started = server.command(
        "trust-lsp.hmiAgent.runSimulatorJourney",
        journey(&project, &draft, &id, cookie, &target, steps),
    );
    assert_eq!(
        started["data"],
        json!({ "started": true, "sessionId": id, "journeyId": "journey-1" }),
        "{started}"
    );
    let ended = server.notification("trust-lsp/simulatorJourneyEnded", WAIT);
    let answer = &ended["params"];
    assert_eq!(answer["sessionId"], id);
    assert_eq!(answer["journeyId"], "journey-1");
    assert_eq!(answer["origin"], "simulator");
    assert_eq!(answer["passed"], true, "{answer}");
    assert_eq!(answer["outcomes"][1]["outcome"], "observed");
    server.stop();
}

/// Catches: journeys that hold the language server's request slots while they wait on a simulator
/// that never answers, so that a cancel, or any other request, waits behind them (14.86 s in the
/// review's probe with four such journeys).
#[test]
fn journeys_waiting_on_a_hung_simulator_leave_the_server_answering() {
    let hung = TcpListener::bind("127.0.0.1:0").expect("a listener that never answers");
    let url = format!("http://{}", hung.local_addr().expect("its address"));
    let project = Project::new("hung");
    let mut server = LanguageServer::start("warn");
    let (draft, _) = project.settings_draft(&mut server);
    let mut sessions = Vec::new();
    for _ in 0..5 {
        let (target, id) = open(&mut server, &project, &url, "access-token-7");
        let started = server.command(
            "trust-lsp.hmiAgent.runSimulatorJourney",
            journey(
                &project,
                &draft,
                &id,
                "trust_hmi_session=4f1c0ffee",
                &target,
                json!([{ "page": "settings" }]),
            ),
        );
        assert_eq!(started["data"]["started"], true, "{started}");
        sessions.push(id);
    }
    let asked = Instant::now();
    let check = json!({ "simulator": true, "url": url, "endpoint": "tcp://127.0.0.1:9100" });
    let local = server.command("trust-lsp.hmiAgent.checkTarget", check);
    let cancelled = server.command(
        "trust-lsp.hmiAgent.cancelSimulatorJourney",
        json!({ "sessionId": sessions[0], "journeyId": "journey-1" }),
    );
    let took = asked.elapsed();
    assert_eq!(local, json!({ "local": true }));
    assert_eq!(cancelled["data"]["cancelled"], true);
    assert!(took < Duration::from_secs(2), "answered in {took:?}");
    server.stop();
    drop(hung);
}

/// Catches: the named session's cookie or the runtime's access token in any log line of the language
/// server, its transport's (tower-lsp, which traces every message it receives and sends unless told
/// not to) or a library's (tungstenite), with all of them at trace level, or in the journey's answer;
/// and a `RUST_LOG` directive that outranks the transport's silence, as a field filter does
/// (`tower_lsp::codec[{message}]=trace` printed the cookie and the token past an `off` directive).
#[test]
fn the_cookie_and_the_token_reach_no_log_line() {
    const COOKIE: &str = "c00kie-5ecret-91";
    const TOKEN: &str = "t0ken-5ecret-37";
    let project = Project::new("log");
    for log in [
        "tower_lsp=trace,tungstenite=trace,trust_lsp=trace",
        "tower_lsp::codec[{message}]=trace",
        "tower_lsp[{message}]=trace",
    ] {
        let mut server = LanguageServer::start(log);
        let (draft, model) = project.settings_draft(&mut server);
        let url = simulator(model);
        let (target, id) = open(&mut server, &project, &url, TOKEN);
        let steps = json!([{ "page": "settings", "object": "stop", "action": "reset" }]);
        let cookie = format!("trust_hmi_session={COOKIE}");
        let started = server.command(
            "trust-lsp.hmiAgent.runSimulatorJourney",
            journey(&project, &draft, &id, &cookie, &target, steps),
        );
        assert_eq!(started["data"]["started"], true, "{log}: {started}");
        let ended = server.notification("trust-lsp/simulatorJourneyEnded", WAIT);
        assert_eq!(
            ended["params"]["passed"], true,
            "{log}: the journey ran: {ended}"
        );
        if log.starts_with("tower_lsp=trace") {
            // Each of the three logs is on: tower-lsp's own, the language server's and tungstenite's.
            let cancel = json!({ "jsonrpc": "2.0", "method": "$/cancelRequest",
                                 "params": { "id": 424242 } });
            server.send(&cancel);
            for line in [
                "client asked to cancel request 424242",
                "HMI agent simulator journey ended",
                "Received message",
            ] {
                assert!(server.logged(line, WAIT), "the log has {line:?}");
            }
        }
        let written = server.stop();
        for secret in [COOKIE, TOKEN] {
            assert!(
                !written.contains(secret),
                "{log}: the log names {secret}: {written}"
            );
            assert!(
                !ended.to_string().contains(secret),
                "the answer names {secret}"
            );
        }
    }
}
