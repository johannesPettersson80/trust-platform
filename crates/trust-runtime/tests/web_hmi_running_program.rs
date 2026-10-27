//! Specification 39, §21.2 and §21.4, on the real `trust-runtime run` in production mode, with
//! debugging disabled as production has it by default (specification 11 §6.9). The project has
//! `Hmi.Display`, a drive whose base implements an interface with methods, and a panel with
//! settings. The test sends `POST /api/hmi/command` and `POST /api/hmi/settings` over HTTP and
//! reads the screen's values from its stream, so the runtime's own wiring is proven: the checks
//! read the running program's image, as nothing gives the web server a description of the
//! project's sources. Every expected value is worked out by hand from the program below.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::fs::{self, File};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};
use tungstenite::{Message, WebSocket};

use runtime_process::{describe, trust_runtime_command};
use support::{http_request, session_cookie, SCREEN};
use trust_runtime::hmi::access::password::hash_password;

/// The HMI library, which the project carries in its sources.
const HMI_LIBRARY: &str = include_str!("../../../libraries/hmi/src/hmi.st");

/// A drive whose base implements `IDrive`, which extends `IDevice`. `Configure` stores its inputs,
/// whose declared initial values are 50.0, T#2s and Manual; `SetMode` sets the mode. `StartMode`
/// and `Gain` are its settings.
const PROGRAM: &str = r#"
TYPE E_Mode : (Auto, Manual, OutOfService); END_TYPE

INTERFACE IDevice
    METHOD SetMode : BOOL
    VAR_INPUT
        Value : E_Mode;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IDrive EXTENDS IDevice
    METHOD Configure : BOOL
    VAR_INPUT
        Speed : LREAL := 50.0;
        Ramp : TIME := T#2s;
        Start : E_Mode := E_Mode#Manual;
    END_VAR
    END_METHOD
    METHOD Rename : BOOL
    VAR_INPUT
        Label : STRING[8];
    END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK DriveBase IMPLEMENTS IDrive
VAR_OUTPUT
    Running : BOOL;
    Mode : E_Mode;
    LastSpeed : LREAL;
    LastRamp : TIME;
    LastStart : E_Mode;
    Configured : DINT;
    Tag : STRING[8];
END_VAR
VAR PUBLIC
    StartMode : E_Mode := E_Mode#Manual;
    Gain : REAL := 1.5;
END_VAR
METHOD PUBLIC SetMode : BOOL
VAR_INPUT
    Value : E_Mode;
END_VAR
    Mode := Value;
    SetMode := TRUE;
END_METHOD
METHOD PUBLIC Configure : BOOL
VAR_INPUT
    Speed : LREAL := 50.0;
    Ramp : TIME := T#2s;
    Start : E_Mode := E_Mode#Manual;
END_VAR
    LastSpeed := Speed;
    LastRamp := Ramp;
    LastStart := Start;
    Configured := Configured + 1;
    Configure := TRUE;
END_METHOD
METHOD PUBLIC Rename : BOOL
VAR_INPUT
    Label : STRING[8];
END_VAR
    Tag := Label;
    Rename := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Drive EXTENDS DriveBase
END_FUNCTION_BLOCK

PROGRAM Plant
VAR
    D1 : Drive;
END_VAR
    D1();
END_PROGRAM

PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION Cell
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Plant;
    TASK ScreenTask (INTERVAL := T#50ms, PRIORITY := 2);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
END_CONFIGURATION
"#;

/// The drive, which opens its panel; a button that configures it with a speed alone; a mode
/// selector that sends the mode chosen; and the values the method stores.
const OVERVIEW: &str = r#"format = 1
title = "Overview"

[[object]]
id = "drive"
tag = "M101"
type = "pump"
block = "Drive"
instance = "Main.D1"
faceplate = "drive-panel"
at = [40, 40]
size = [128, 108]

[object.bind]
running = "Main.D1.Running"

[[object]]
id = "configure"
type = "button"
text = "Configure"
at = [240, 40]
size = [160, 48]

[object.command]
interface = "IDrive"
method = "Configure"
instance = "Main.D1"

[object.command.arguments]
Speed = 75.5

[[object]]
id = "rename"
type = "button"
text = "Rename"
at = [420, 40]
size = [160, 48]

[object.command]
interface = "IDrive"
method = "Rename"
instance = "Main.D1"

[object.command.arguments]
Label = "Pümp 101"

[[object]]
id = "mode"
type = "mode-selector"
options = ["Auto", "Manual"]
at = [240, 120]
size = [320, 48]

[object.bind]
mode = "Main.D1.Mode"

[object.command]
interface = "IDevice"
method = "SetMode"
instance = "Main.D1"

[object.command.arguments]
Value = { input = true }

[[object]]
id = "speed"
type = "value"
label = "Speed"
at = [40, 200]
size = [240, 96]

[object.bind]
value = "Main.D1.LastSpeed"

[[object]]
id = "ramp"
type = "value"
label = "Ramp"
at = [300, 200]
size = [240, 96]

[object.bind]
value = "Main.D1.LastRamp"

[[object]]
id = "start"
type = "value"
label = "Start"
at = [560, 200]
size = [240, 96]

[object.bind]
value = "Main.D1.LastStart"

[[object]]
id = "configured"
type = "value"
label = "Configured"
at = [820, 200]
size = [240, 96]

[object.bind]
value = "Main.D1.Configured"

[[object]]
id = "tag"
type = "text"
at = [40, 320]
size = [240, 40]

[object.bind]
text = "Main.D1.Tag"
"#;

/// The drive's panel, with the Settings dialog.
const PANEL: &str = r#"format = 1
title = "Drive"
size = [400, 200]
settings = true

[fields.Running]
accepts = ["boolean"]

[[object]]
id = "state"
type = "status"
label = "Running"
at = [24, 24]
size = [272, 40]

[object.bind]
state = "$Running"
"#;

/// The drive's component definition: an enumeration setting and a REAL setting from 0 to 10.
const PAIR: &str = r#"format = 1
title = "Drive"
block = "Drive"
faceplate = "drive-panel"
symbol = "pump"

[[settings]]
member = "StartMode"
title = "Start mode"
key = true

[[settings]]
member = "Gain"
title = "Gain"
min = 0
max = 10
"#;

/// How the Settings dialog names the drive's panel.
const DRIVE_PANEL: &str = "faceplate:overview/drive";
/// The longest wait for one observable change.
const WAIT: Duration = Duration::from_secs(20);

/// The running `trust-runtime run` of a project folder: killed when dropped, and the folder
/// removed.
struct Runtime {
    root: PathBuf,
    child: Child,
    base: String,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Runtime {
    /// The runtime's output, for failure messages.
    fn diagnostics(&self) -> String {
        let log = |name: &str| {
            fs::read_to_string(self.root.join(name)).unwrap_or_else(|_| "<not written>".into())
        };
        format!(
            "runtime stdout:\n{}\nruntime stderr:\n{}",
            log("runtime.stdout.log"),
            log("runtime.stderr.log")
        )
    }
}

/// Two free local ports, both held until both are known, so they differ.
fn free_ports() -> (u16, u16) {
    let first = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let second = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let port = |listener: &TcpListener| listener.local_addr().expect("the local address").port();
    (port(&first), port(&second))
}

fn write(root: &Path, file: &str, text: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().expect("a parent folder")).expect("create folders");
    fs::write(path, text).expect("write a project file");
}

/// The project in a fresh folder, built with `trust-runtime build` and run with `trust-runtime run`
/// in production mode with debugging disabled; its web interface answers when this returns.
fn start() -> Runtime {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-hmi-running-program-{}-{stamp}",
        std::process::id()
    ));
    let (control, web) = free_ports();
    let password = hash_password("secret", 10_000, b"0123456789abcdef");
    write(
        &root,
        "runtime.toml",
        &format!(
            r#"[bundle]
version = 1

[resource]
name = "Cpu"
cycle_interval_ms = 20

[runtime.control]
endpoint = "tcp://127.0.0.1:{control}"
mode = "production"
debug_enabled = false
auth_token = "hmi-running-program-{stamp}"

[runtime.web]
enabled = true
listen = "127.0.0.1:{web}"
auth = "local"
tls = false

[runtime.web.hmi]
idle_minutes = 15

[[runtime.web.hmi.user]]
name = "operator"
role = "operator"
password = "{password}"

[[runtime.web.hmi.user]]
name = "engineer"
role = "engineer"
password = "{password}"

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = []

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"#
        ),
    );
    write(
        &root,
        "io.toml",
        "[io]\ndriver = \"simulated\"\nparams = {}\n",
    );
    write(
        &root,
        "trust-lsp.toml",
        "[project]\ninclude_paths = [\"src\"]\n",
    );
    write(&root, "src/hmi.st", HMI_LIBRARY);
    write(&root, "src/main.st", PROGRAM);
    write(&root, "hmi/screen.toml", SCREEN);
    write(&root, "hmi/pages/overview.toml", OVERVIEW);
    write(&root, "hmi/panels/drive-panel.toml", PANEL);
    write(&root, "hmi/pairs/drive.toml", PAIR);
    let built = trust_runtime_command()
        .arg("build")
        .arg("--project")
        .arg(&root)
        .output()
        .expect("run trust-runtime build");
    assert!(
        built.status.success(),
        "trust-runtime build failed\n{}",
        describe(&built)
    );
    let stdout = File::create(root.join("runtime.stdout.log")).expect("stdout log");
    let stderr = File::create(root.join("runtime.stderr.log")).expect("stderr log");
    let child = trust_runtime_command()
        .arg("run")
        .arg("--project")
        .arg(&root)
        .env_remove("TRUST_CTL_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn trust-runtime run");
    let mut runtime = Runtime {
        root,
        child,
        base: format!("http://127.0.0.1:{web}"),
    };
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = runtime.child.try_wait().expect("poll the runtime") {
            panic!(
                "the runtime exited before its web interface answered: {status}\n{}",
                runtime.diagnostics()
            );
        }
        if ureq::get(&format!("{}/", runtime.base)).call().is_ok() {
            return runtime;
        }
        assert!(
            Instant::now() < deadline,
            "the web interface did not answer within {WAIT:?}\n{}",
            runtime.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The version of the screens' files, which every command names.
fn bundle(runtime: &Runtime) -> Value {
    let (status, data, _) = http_request(
        "GET",
        &runtime.base,
        "/api/hmi/popup?faceplate=overview/drive",
        None,
        None,
    );
    assert_eq!(status, 200, "{data}");
    assert!(data["bundle"].is_string(), "{data}");
    data["bundle"].clone()
}

/// Sends `invoke` for an object of the overview, with the value its control sends.
fn invoke(runtime: &Runtime, cookie: &str, id: &str, object: &str, value: Option<Value>) -> Value {
    let mut body = json!({
        "id": id,
        "bundle": bundle(runtime),
        "page": "overview",
        "object": object,
        "action": "invoke",
    });
    if let Some(value) = value {
        body["value"] = value;
    }
    let (status, answer, _) = http_request(
        "POST",
        &runtime.base,
        "/api/hmi/command",
        Some(cookie),
        Some(body),
    );
    assert_eq!(status, 200, "{answer}");
    answer
}

fn settings(runtime: &Runtime, cookie: &str, body: Value) -> Value {
    let (status, answer, _) = http_request(
        "POST",
        &runtime.base,
        "/api/hmi/settings",
        Some(cookie),
        Some(body),
    );
    assert_eq!(status, 200, "{answer}");
    answer
}

/// The value the Settings dialog shows for `member`.
fn setting(answer: &Value, member: &str) -> Value {
    answer["settings"]
        .as_array()
        .and_then(|settings| settings.iter().find(|setting| setting["member"] == member))
        .map(|setting| setting["value"].clone())
        .unwrap_or_else(|| panic!("{member} in {answer}"))
}

/// The overview's value stream: the newest value of each path it shows.
struct Values {
    socket: WebSocket<TcpStream>,
    newest: Map<String, Value>,
}

impl Values {
    fn open(base: &str) -> Self {
        let address = base.trim_start_matches("http://").to_string();
        let stream = TcpStream::connect(&address).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("timeout");
        let (socket, _) =
            tungstenite::client(format!("ws://{address}/ws/hmi?page=overview"), stream)
                .expect("the overview's stream opens");
        Self {
            socket,
            newest: Map::new(),
        }
    }

    fn read(&mut self) {
        if let Ok(Message::Text(text)) = self.socket.read() {
            let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
            if let Some(Value::Object(values)) = message.get("values") {
                for (path, value) in values {
                    self.newest.insert(path.clone(), value["value"].clone());
                }
            }
        }
    }

    /// The newest value of `path` after following the stream for `millis`.
    fn after(&mut self, path: &str, millis: u64) -> Value {
        let deadline = Instant::now() + Duration::from_millis(millis);
        while Instant::now() < deadline {
            self.read();
        }
        self.newest.get(path).cloned().unwrap_or(Value::Null)
    }

    /// Whether every path of `expected` reads its value within the wait.
    fn reach(&mut self, expected: &[(&str, Value)]) -> bool {
        let deadline = Instant::now() + WAIT;
        loop {
            if expected
                .iter()
                .all(|(path, value)| self.newest.get(*path) == Some(value))
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            self.read();
        }
    }
}

#[test]
fn the_real_runtime_checks_method_commands_and_settings_against_the_program_it_runs() {
    // §21.2 item 5 and §21.4 item 4 (specification 44 §5.4, §5.7, §5.8, §13.3). The checks read
    // the running program's image, so on the real runtime, where the web server has no
    // description of the sources and debugging is off:
    // - Configure, whose button gives only Speed = 75.5, is accepted: Drive implements IDrive
    //   through its base, and the program gives the inputs the button leaves out their declared
    //   initial values, so the drive stores 75.5, T#2s and Manual, once;
    // - the mode selector's Manual reaches SetMode, a method of IDrive's base IDevice, as the
    //   E_Mode value Manual;
    // - Rename's button sends "Pümp 101", 8 characters but 9 bytes of UTF-8, to a STRING[8]
    //   input: refused before the call with `runtime_string_capacity_exceeded` (§12.8 item 5), so
    //   the drive's Tag stays empty, never cut to "Pümp 10";
    // - the Settings dialog writes Auto, a value of the enumeration E_Mode, and 2.5 into the REAL
    //   Gain, within its 0 to 10, and refuses Sideways, which E_Mode does not declare, naming the
    //   setting, so StartMode keeps Auto.
    // Catches the checks reading the empty description `trust-runtime run` gives the web server
    // ("Drive does not implement IDrive", "the values of E_Mode are not known"), checks that need
    // debugging enabled ("the equipment is not available"), an HMI that sends inputs of its own
    // for those a command leaves out, and a text cut to its input or refused without its code.
    let runtime = start();
    let mut values = Values::open(&runtime.base);
    let operator = session_cookie(&runtime.base, "operator");

    let configured = invoke(&runtime, &operator, "c-configure", "configure", None);
    assert_eq!(
        configured["outcome"],
        json!("applied"),
        "{configured}\n{}",
        runtime.diagnostics()
    );
    assert!(
        values.reach(&[
            ("Main.D1.LastSpeed", json!(75.5)),
            ("Main.D1.LastRamp", json!("T#2s")),
            ("Main.D1.LastStart", json!("Manual")),
            ("Main.D1.Configured", json!(1)),
        ]),
        "the drive stores the speed and the declared initial values: {:?}",
        values.newest
    );

    let manual = invoke(
        &runtime,
        &operator,
        "c-manual",
        "mode",
        Some(json!("Manual")),
    );
    assert_eq!(manual["outcome"], json!("applied"), "{manual}");
    assert!(
        values.reach(&[("Main.D1.Mode", json!("Manual"))]),
        "the drive's mode: {:?}",
        values.newest
    );

    let renamed = invoke(&runtime, &operator, "c-rename", "rename", None);
    assert_eq!(
        (
            &renamed["outcome"],
            &renamed["error_code"],
            &renamed["reason"]
        ),
        (
            &json!("refused"),
            &json!("runtime_string_capacity_exceeded"),
            &json!("\"Label\" holds at most 8 bytes of UTF-8; the text has 9")
        ),
        "{renamed}"
    );
    assert_eq!(
        values.after("Main.D1.Tag", 500),
        json!(""),
        "nothing reached the drive: {:?}",
        values.newest
    );

    let engineer = session_cookie(&runtime.base, "engineer");
    let read = settings(
        &runtime,
        &engineer,
        json!({ "faceplate": DRIVE_PANEL, "action": "read" }),
    );
    assert_eq!(read["ok"], json!(true), "{read}");
    assert_eq!(setting(&read, "StartMode"), json!("Manual"), "as declared");
    assert_eq!(setting(&read, "Gain"), json!(1.5), "as declared");
    let applied = settings(
        &runtime,
        &engineer,
        json!({
            "faceplate": DRIVE_PANEL,
            "action": "apply",
            "changes": { "StartMode": "Auto", "Gain": 2.5 },
            "expected": { "StartMode": "Manual", "Gain": 1.5 },
        }),
    );
    assert_eq!(applied["ok"], json!(true), "{applied}");
    assert_eq!(setting(&applied, "StartMode"), json!("Auto"));
    assert_eq!(setting(&applied, "Gain"), json!(2.5));
    let refused = settings(
        &runtime,
        &engineer,
        json!({
            "faceplate": DRIVE_PANEL,
            "action": "apply",
            "changes": { "StartMode": "Sideways" },
            "expected": { "StartMode": "Auto" },
        }),
    );
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(refused["field"], json!("StartMode"), "{refused}");
    assert_eq!(
        refused["error"],
        json!("Start mode: \"Sideways\" is not a value of E_Mode"),
        "{refused}"
    );
    let again = settings(
        &runtime,
        &engineer,
        json!({ "faceplate": DRIVE_PANEL, "action": "read" }),
    );
    assert_eq!(
        setting(&again, "StartMode"),
        json!("Auto"),
        "nothing written"
    );
}
