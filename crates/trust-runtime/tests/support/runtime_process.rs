//! Shared harness for the `ctl` workflow tests. It runs the real `trust-runtime` binary on a
//! private copy of an example project, so no test writes into the repository and the control
//! endpoint is a real TCP socket. Every fixed listen port in the copy moves to a reserved free
//! loopback port, so tests that run in parallel cannot collide. Included by each test file with
//! `#[path = "support/runtime_process.rs"] mod runtime_process;`.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// Prefix of the token written into each copied runtime.toml; `ctl` reads it from the project.
/// Every copy gets its own token, so a client can never be served by another test's runtime.
const TOKEN_PREFIX: &str = "runtime-ctl-journey-token";
/// Upper bound for one observable change. A scan is 50 ms, so this is generous.
pub const WAIT: Duration = Duration::from_secs(10);
/// Upper bound for the counter to climb past a value. At 50 ms per scan, 44 scans take about
/// 2.2 s; the extra room is for slow runners.
pub const CLIMB: Duration = Duration::from_secs(30);
/// The control endpoint must answer `ctl status` within this time after `play` starts.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// Interval between polls. It is shorter than the 50 ms example cycle, so consecutive scans are
/// seen rather than skipped.
const POLL: Duration = Duration::from_millis(25);
/// Entries left out of the copy: bundles from earlier builds and dependency caches.
const SKIPPED: [&str; 4] = ["program.stbc", "target", ".trust", "node_modules"];
/// Fixed listen addresses in the example projects, by runtime.toml table and key. Each one moves
/// to a reserved loopback port in the copy. The examples' io.toml files need no change: their only
/// `address` keys are process-image addresses such as `%QX0.0`, not network ports.
const LISTEN_KEYS: [(&str, &str); 3] = [
    ("runtime.web", "listen"),
    ("runtime.mesh", "listen"),
    ("runtime.opcua", "listen"),
];

static COPY_COUNTER: AtomicU64 = AtomicU64::new(1);
static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);
/// Ports this test process has handed out. A port is never given to two runtimes, so runtimes
/// started in parallel cannot race for a port that an earlier reservation released.
static HANDED_OUT_PORTS: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// The repository root, located from this crate's manifest directory.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A fresh `trust-runtime` command, ready for arguments.
pub fn trust_runtime_command() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
}

/// Status, stdout and stderr of a finished command, for assertion messages.
pub fn describe(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Sleeps for a fixed window. Only for asserting that something does NOT change: callers first
/// wait for a steady state, then hold for a few 50 ms scans and compare the counter.
pub fn hold_for(window: Duration) {
    std::thread::sleep(window);
}

/// Describes a project copy and the runtime settings a test needs. Settings left unset keep the
/// example's own runtime.toml values.
pub struct ExampleRuntime {
    example: &'static str,
    libraries: Vec<&'static str>,
    source_edits: Vec<(&'static str, &'static str, &'static str)>,
    mode: Option<&'static str>,
    debug_enabled: Option<bool>,
}

impl ExampleRuntime {
    /// `example` is a path under `examples/`, such as `memory_marker_counter`.
    pub fn new(example: &'static str) -> Self {
        Self {
            example,
            libraries: Vec::new(),
            source_edits: Vec::new(),
            mode: None,
            debug_enabled: None,
        }
    }

    /// Also copies a repository directory that the project depends on, to the same relative path
    /// in the copy. `trust-lsp.toml` path dependencies resolve against the project.
    pub fn library(mut self, relative_dir: &'static str) -> Self {
        self.libraries.push(relative_dir);
        self
    }

    /// Replaces `from` with `to` in one file of the copy, before the project is built.
    pub fn edit_source(mut self, file: &'static str, from: &'static str, to: &'static str) -> Self {
        self.source_edits.push((file, from, to));
        self
    }

    /// Sets `runtime.control.mode` (`"production"` or `"debug"`).
    pub fn mode(mut self, mode: &'static str) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Sets `runtime.control.debug_enabled`.
    pub fn debug_enabled(mut self, enabled: bool) -> Self {
        self.debug_enabled = Some(enabled);
        self
    }

    /// Copies, builds and starts the runtime, then waits until its control endpoint answers.
    pub fn start(self) -> Runtime {
        let root = unique_root();
        let project = join_relative(&root, &format!("examples/{}", self.example));
        // The runtime owns the root from here on, so a failed step still removes the copy.
        let mut runtime = Runtime {
            root: root.clone(),
            project: project.clone(),
            child: None,
        };
        let source = join_relative(&repo_root(), &format!("examples/{}", self.example));
        copy_tree(&source, &project);
        for &dir in &self.libraries {
            copy_tree(
                &join_relative(&repo_root(), dir),
                &join_relative(&root, dir),
            );
        }
        for &(file, from, to) in &self.source_edits {
            replace_in_file(&project.join(file), from, to);
        }
        // One reserved port for the control endpoint, then one for each fixed listen key.
        let ports = reserve_local_ports(1 + LISTEN_KEYS.len());
        let token = format!(
            "{TOKEN_PREFIX}-{}-{}",
            std::process::id(),
            TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        configure_runtime_toml(
            &project.join("runtime.toml"),
            &ports,
            &token,
            self.mode,
            self.debug_enabled,
        );
        runtime.build();
        runtime.spawn_play();
        runtime.await_control();
        runtime
    }
}

/// A running `trust-runtime play` on a private project copy. Dropping it kills the process if it
/// is still running and removes the copy.
pub struct Runtime {
    root: PathBuf,
    project: PathBuf,
    child: Option<Child>,
}

/// What `prove_counter_write` established: the first snapshot the write changed, and the scan
/// whose cycle start took the written value.
pub struct CounterWrite {
    pub after: Snapshot,
    pub landing_scan: i64,
}

impl Runtime {
    pub fn project(&self) -> &Path {
        &self.project
    }

    /// The tutorials' build step. `play` on a folder without `program.stbc` takes the
    /// project-creation path instead (run/util.rs, `should_auto_create`), which can rewrite the
    /// copied sources, so the copy is built explicitly first.
    fn build(&self) {
        let output = trust_runtime_command()
            .arg("build")
            .arg("--project")
            .arg(&self.project)
            .output()
            .expect("run trust-runtime build");
        assert!(
            output.status.success(),
            "trust-runtime build failed\n{}",
            describe(&output)
        );
    }

    fn spawn_play(&mut self) {
        let stdout = File::create(self.root.join("runtime.stdout.log")).expect("stdout log");
        let stderr = File::create(self.root.join("runtime.stderr.log")).expect("stderr log");
        let child = trust_runtime_command()
            .arg("play")
            .arg("--project")
            .arg(&self.project)
            .arg("--no-console")
            .env_remove("TRUST_CTL_TOKEN")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("spawn trust-runtime play");
        self.child = Some(child);
    }

    fn await_control(&mut self) {
        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            let exited = self
                .child
                .as_mut()
                .expect("runtime child")
                .try_wait()
                .expect("poll runtime");
            if let Some(status) = exited {
                panic!(
                    "runtime exited before its control endpoint answered: {status}\n{}",
                    self.diagnostics()
                );
            }
            if self.ctl(&["status"]).status.success() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "control endpoint did not answer within {START_TIMEOUT:?}\n{}",
                self.diagnostics()
            );
            std::thread::sleep(POLL);
        }
    }

    /// Runs `trust-runtime ctl --project <project> <args>`.
    pub fn ctl(&self, args: &[&str]) -> Output {
        trust_runtime_command()
            .arg("ctl")
            .arg("--project")
            .arg(&self.project)
            .args(args)
            .env_remove("TRUST_CTL_TOKEN")
            .output()
            .expect("run trust-runtime ctl")
    }

    /// Like `ctl`, but asserts exit 0 and returns stdout.
    pub fn ctl_ok(&self, args: &[&str]) -> String {
        let output = self.ctl(args);
        assert!(
            output.status.success(),
            "`ctl {}` failed\n{}",
            args.join(" "),
            describe(&output)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// `ctl_ok` for the commands that print the raw JSON response: returns its `result`.
    pub fn ctl_result(&self, args: &[&str]) -> Value {
        let stdout = self.ctl_ok(args);
        let response: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|err| {
            panic!(
                "`ctl {}` printed no JSON response ({err}): {stdout}",
                args.join(" ")
            )
        });
        assert_eq!(
            response["ok"],
            true,
            "`ctl {}` response: {response}",
            args.join(" ")
        );
        response["result"].clone()
    }

    /// The parsed `ctl status` summary, asserted to be in the documented form.
    pub fn status(&self) -> StatusSummary {
        parse_status_summary(&self.ctl_ok(&["status"]))
    }

    /// The latest snapshot from `ctl io-read`, or None before the first scan.
    pub fn snapshot(&self) -> Option<Snapshot> {
        let result = self.ctl_result(&["io-read"]);
        let snapshot = result
            .get("snapshot")
            .expect("io-read result has a snapshot field");
        (!snapshot.is_null()).then(|| Snapshot(snapshot.clone()))
    }

    /// The scan counter of the latest snapshot. Panics before the first scan.
    pub fn scan(&self) -> u64 {
        self.snapshot()
            .expect("an I/O snapshot after the first scan")
            .scan()
    }

    /// Polls until `condition` holds, failing with `what` and the runtime's logs on timeout.
    pub fn wait_until(&self, timeout: Duration, what: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + timeout;
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "timed out after {timeout:?} waiting for {what}\n{}",
                self.diagnostics()
            );
            std::thread::sleep(POLL);
        }
    }

    /// Polls until a snapshot satisfies `accept`, and returns that snapshot. The caller then checks
    /// one coherent scan, not values read by two separate commands.
    pub fn wait_for_snapshot(
        &self,
        timeout: Duration,
        what: &str,
        mut accept: impl FnMut(&Snapshot) -> bool,
    ) -> Snapshot {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(snapshot) = self.snapshot()
                && accept(&snapshot)
            {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "timed out after {timeout:?} waiting for {what}\n{}",
                self.diagnostics()
            );
            std::thread::sleep(POLL);
        }
    }

    /// Queues `io-write %MW0 <written>` on a counter that climbs by `step` per scan, and proves by
    /// arithmetic that the write landed. No time window decides the result.
    ///
    /// While the counter climbs, the offset `%MW0 - step * scan` stays constant. The write sets the
    /// counter to `written` at its landing scan k, and it climbs from `written + step` after that.
    /// So a later snapshot `s` satisfies `%MW0(s) = written + step + step * (s - k)`. With
    /// `delta = %MW0(s) - (written + step)`, the landing scan is `k = s - delta / step`. A write
    /// that is ignored never changes the offset, so the wait for that change times out and fails.
    pub fn prove_counter_write(&self, written: i64, step: i64) -> CounterWrite {
        let before = self.wait_for_snapshot(CLIMB, "the counter past the written value", |s| {
            s.int("%MW0") > written + 2 * step
        });
        let offset = before.int("%MW0") - step * before.scan() as i64;
        let value = written.to_string();
        let queued = self.ctl_ok(&["io-write", "%MW0", value.as_str()]);
        assert!(
            queued.contains("queued"),
            "io-write must queue the write: {queued}"
        );
        let after = self.wait_for_snapshot(WAIT, "the write to change the counter's offset", |s| {
            s.int("%MW0") - step * s.scan() as i64 != offset
        });
        let delta = after.int("%MW0") - (written + step);
        assert!(
            delta >= 0 && delta % step == 0,
            "after the write %MW0 is {}, which is not {written} + {step} plus whole steps",
            after.int("%MW0")
        );
        let landing_scan = after.scan() as i64 - delta / step;
        assert!(
            (before.scan() as i64) < landing_scan && landing_scan <= after.scan() as i64,
            "the write landed on scan {landing_scan}, outside ({}, {}]",
            before.scan(),
            after.scan()
        );
        CounterWrite {
            after,
            landing_scan,
        }
    }

    /// Whether the runtime process is still alive.
    pub fn is_running(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }

    /// Waits for the process to exit on its own, as after `shutdown`.
    pub fn wait_for_exit(&mut self, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            let exited = self
                .child
                .as_mut()
                .expect("runtime child")
                .try_wait()
                .expect("poll runtime");
            if let Some(status) = exited {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "runtime did not exit within {timeout:?}\n{}",
                self.diagnostics()
            );
            std::thread::sleep(POLL);
        }
    }

    /// The PLC runs with no fault and its process is alive. This is the state that #135 and #136
    /// broke.
    pub fn assert_running_without_fault(&mut self) {
        let status = self.status();
        assert_eq!(status.state, "running", "{}", self.diagnostics());
        assert_eq!(status.fault, "none", "{}", self.diagnostics());
        assert!(
            self.is_running(),
            "the runtime process exited\n{}",
            self.diagnostics()
        );
    }

    /// The runtime's captured stdout and stderr, for failure messages.
    pub fn diagnostics(&self) -> String {
        format!(
            "runtime stdout:\n{}\nruntime stderr:\n{}",
            read_log(&self.root.join("runtime.stdout.log")),
            read_log(&self.root.join("runtime.stderr.log"))
        )
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if matches!(child.try_wait(), Ok(None)) {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// One coherent `io-read` snapshot: every value in it comes from the same scan.
pub struct Snapshot(Value);

impl Snapshot {
    pub fn scan(&self) -> u64 {
        self.0["scan"].as_u64().expect("snapshot scan number")
    }

    fn entry(&self, address: &str) -> &Value {
        ["inputs", "outputs", "memory"]
            .into_iter()
            .filter_map(|area| self.0[area].as_array())
            .flatten()
            .find(|entry| entry["address"] == address)
            .unwrap_or_else(|| panic!("snapshot has no entry for {address}: {}", self.0))
    }

    /// The printed value of an entry, such as `Int(41)` or `Bool(true)`.
    pub fn text(&self, address: &str) -> String {
        let entry = self.entry(address);
        entry["value"]
            .as_str()
            .unwrap_or_else(|| panic!("{address} has no printed value: {entry}"))
            .to_string()
    }

    /// The integer of an entry, read from its printed form (`Int(41)` gives 41).
    pub fn int(&self, address: &str) -> i64 {
        let text = self.text(address);
        text.strip_suffix(')')
            .and_then(|rest| rest.split_once('('))
            .and_then(|(_, inner)| inner.parse().ok())
            .unwrap_or_else(|| panic!("{address} is not an integer: {text}"))
    }

    pub fn forced(&self, address: &str) -> bool {
        self.entry(address)["forced"]
            .as_bool()
            .expect("snapshot entry forced flag")
    }
}

/// The `ctl status` line: `state=<state> fault=<fault> rt_profile=<profile> rt_active=<bool>`.
pub struct StatusSummary {
    pub state: String,
    pub fault: String,
    pub rt_profile: String,
    pub rt_active: String,
}

/// Parses the summary in the documented form (spec 11 §6.8.1). Any other shape fails.
pub fn parse_status_summary(line: &str) -> StatusSummary {
    let line = line.trim_end();
    let fields: Vec<(&str, &str)> = line
        .split(' ')
        .map(|field| {
            field
                .split_once('=')
                .unwrap_or_else(|| panic!("status field `{field}` is not key=value in `{line}`"))
        })
        .collect();
    let keys: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        keys,
        ["state", "fault", "rt_profile", "rt_active"],
        "status must print the documented summary: `{line}`"
    );
    assert!(
        matches!(fields[3].1, "true" | "false"),
        "rt_active must be a boolean: `{line}`"
    );
    StatusSummary {
        state: fields[0].1.to_string(),
        fault: fields[1].1.to_string(),
        rt_profile: fields[2].1.to_string(),
        rt_active: fields[3].1.to_string(),
    }
}

fn unique_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let sequence = COPY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-ctl-{}-{nanos}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create runtime test directory");
    root
}

/// Joins a `/`-separated repository path onto `base`, one component at a time.
fn join_relative(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

/// Distinct free loopback ports that no earlier reservation in this process handed out. Every
/// listener, including one whose port was already handed out, is held until the reservation is
/// complete, so the kernel cannot offer the same port twice; they are released together afterwards.
fn reserve_local_ports(count: usize) -> Vec<u16> {
    let mut handed_out = HANDED_OUT_PORTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let mut held = Vec::new();
    let mut ports = Vec::with_capacity(count);
    while ports.len() < count {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
        let port = listener.local_addr().expect("local address").port();
        if handed_out.insert(port) {
            ports.push(port);
        }
        held.push(listener);
    }
    ports
}

/// Copies a directory tree, leaving out build outputs.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copied directory");
    let entries = fs::read_dir(from).unwrap_or_else(|err| panic!("read {}: {err}", from.display()));
    for entry in entries {
        let entry = entry.expect("read directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if SKIPPED.contains(&name.as_str()) {
            continue;
        }
        let source = entry.path();
        let target = to.join(&name);
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&source, &target);
        } else {
            fs::copy(&source, &target)
                .unwrap_or_else(|err| panic!("copy {}: {err}", source.display()));
        }
    }
}

fn replace_in_file(path: &Path, from: &str, to: &str) {
    let text =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    // A Windows checkout can convert the example sources to CRLF line endings, while the edits are
    // written with LF; Structured Text accepts either, so the copy is normalized to LF first.
    let text = text.replace("\r\n", "\n");
    assert!(
        text.contains(from),
        "{} has no `{from}` to edit",
        path.display()
    );
    fs::write(path, text.replace(from, to)).expect("write edited source");
}

/// Rewrites the copied runtime.toml with the toml crate. `ports[0]` becomes the TCP control
/// endpoint, and each `LISTEN_KEYS` entry that exists takes the next reserved port. The copy's own
/// token is set, the mode and debug flag are set when requested, and web and discovery are disabled.
/// Every other setting is kept.
fn configure_runtime_toml(
    path: &Path,
    ports: &[u16],
    token: &str,
    mode: Option<&str>,
    debug_enabled: Option<bool>,
) {
    let text = fs::read_to_string(path).expect("read runtime.toml");
    let mut document: toml::Table = toml::from_str(&text).expect("parse runtime.toml");
    let runtime = table_section(&mut document, "runtime");
    let control = table_section(runtime, "control");
    control.insert(
        "endpoint".into(),
        toml::Value::String(format!("tcp://127.0.0.1:{}", ports[0])),
    );
    control.insert("auth_token".into(), toml::Value::String(token.into()));
    if let Some(mode) = mode {
        control.insert("mode".into(), toml::Value::String(mode.into()));
    }
    if let Some(enabled) = debug_enabled {
        control.insert("debug_enabled".into(), toml::Value::Boolean(enabled));
    }
    for section in ["web", "discovery"] {
        if let Some(table) = runtime.get_mut(section).and_then(toml::Value::as_table_mut) {
            table.insert("enabled".into(), toml::Value::Boolean(false));
        }
    }
    for (&(table, key), &port) in LISTEN_KEYS.iter().zip(&ports[1..]) {
        if let Some(section) = table_path_mut(&mut document, table)
            && section.contains_key(key)
        {
            section.insert(key.into(), toml::Value::String(format!("127.0.0.1:{port}")));
        }
    }
    fs::write(
        path,
        toml::to_string(&document).expect("render runtime.toml"),
    )
    .expect("write runtime.toml");
}

/// The table under `key`, created when absent.
fn table_section<'a>(table: &'a mut toml::Table, key: &str) -> &'a mut toml::Table {
    if !table.contains_key(key) {
        table.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    table
        .get_mut(key)
        .and_then(toml::Value::as_table_mut)
        .expect("runtime.toml section is a table")
}

/// The table at a dotted path such as `runtime.web`, when every segment exists.
fn table_path_mut<'a>(document: &'a mut toml::Table, path: &str) -> Option<&'a mut toml::Table> {
    let mut table = document;
    for segment in path.split('.') {
        table = table.get_mut(segment)?.as_table_mut()?;
    }
    Some(table)
}

fn read_log(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| String::from("<not written>"))
}
