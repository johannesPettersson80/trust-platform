//! A compiled program on a computer (specification 44 §8.8, NX-14 and NX-15): trust-runtime runs it
//! with the chips' compiled cycle, keeps its retained values in the retain file the bytecode engine
//! writes, changes it online, steps it, runs it on the resource's scan thread, and its services read
//! it through the symbol table (§13.3).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::time::{Duration as WallDuration, Instant};

use smol_str::SmolStr;
use trust_runtime::harness::SourceFile;
use trust_runtime::io::IoAddress;
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::retain::FileRetainStore;
use trust_runtime::scheduler::{ResourceCommand, ResourceRunner, StdClock, StepRequest};
use trust_runtime::value::{Duration, Value};
use trust_runtime::{CompiledRuntime, RestartMode};

/// The running program: every scan adds 1 to `count` and 2 to the retained `kept`, and the lamp at
/// `%QX0.0` is on when `count` is odd.
const RUNNING: &str = "
PROGRAM Main
VAR
    count : DINT;
    lamp AT %QX0.0 : BOOL;
END_VAR
VAR RETAIN
    kept : DINT;
END_VAR
count := count + 1;
kept := kept + 2;
lamp := (count MOD 2) = 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// The changed program: `added` is new with 7, and `kept` grows by 3.
const CHANGED: &str = "
PROGRAM Main
VAR
    count : DINT;
    lamp AT %QX0.0 : BOOL;
    added : INT := 7;
END_VAR
VAR RETAIN
    kept : DINT;
END_VAR
count := count + 1;
kept := kept + 3;
lamp := (count MOD 2) = 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// A program whose variables the services list: a global, and a program instance's variables of an
/// elementary type, a string, an enumeration, an array, a timer and a real.
const LISTED: &str = "
TYPE Mode : (Idle, Run, Fault); END_TYPE

VAR_GLOBAL
    Speed : INT := 5;
END_VAR

PROGRAM Main
VAR
    count : DINT;
    label : STRING[20] := 'vat';
    mode : Mode := Mode#Run;
    history : ARRAY[1..3] OF INT := [1, 2, 3];
    timer : TON;
    ratio : REAL := 1.5;
END_VAR
count := count + 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// The image of `source` for this computer.
fn image(source: &str, name: &str) -> Vec<u8> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!("trust-host-{name}-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    built.image
}

/// The program's value of `name`, read from the state its last scan left.
fn value(runtime: &CompiledRuntime, name: &str) -> Option<Value> {
    runtime
        .snapshot()
        .expect("the program keeps its image")
        .value(name)
}

fn millis(ms: i64) -> Duration {
    Duration::from_millis(ms)
}

/// Specification 44 §8.8 with specification 11 §6.7 and §12, worked by hand from the two sources
/// and the periodic rule of specification 10 §4.3 (a task is due once `INTERVAL` has passed since
/// its last run, the start counting as one). Scans at 0, 10, 20 and 30 ms run the 10 ms task at 10,
/// 20 and 30 ms, which leaves `count` 3, `kept` 6 and the lamp on (3 is odd); the retain file, saved
/// at every changed scan, then holds `kept` 6. A second runtime of the same image, restarted warm
/// from that file at 0 ms, starts with `kept` 6 and `count` 0. Changed online, it keeps both and
/// `added` takes 7; a scan at 10 ms, when the task's schedule makes it due, then makes `count` 1 and
/// `kept` 9. A cold restart gives 0 and 0. Catches a cycle that does not run the task, an output the scan does not set, a retain file
/// the computer does not write or does not read at a warm restart, a change that loses the running
/// values or does not run the new code, and a cold restart that keeps them.
#[test]
fn a_computer_runs_a_compiled_program_with_its_retain_file_and_changes_it_online() {
    let running = image(RUNNING, "running");
    let changed = image(CHANGED, "changed");
    let retain = std::env::temp_dir().join(format!("trust-host-retain-{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&retain);

    let mut first = CompiledRuntime::load(running.clone(), "Cpu").expect("the image loads");
    first
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&retain))),
            Some(Duration::ZERO),
        )
        .expect("the retain file is set");
    first.restart(RestartMode::Cold).expect("the cold start");
    first.reset_task_timing(Duration::ZERO);
    for at in [0, 10, 20, 30] {
        first.set_current_time(millis(at));
        first.execute_cycle().expect("the scan runs");
    }
    assert_eq!(value(&first, "P.count"), Some(Value::DInt(3)));
    assert_eq!(value(&first, "P.kept"), Some(Value::DInt(6)));
    assert_eq!(
        first.read_io(&IoAddress::parse("%QX0.0").expect("an address")),
        Ok(Value::Bool(true))
    );
    drop(first);

    let mut second = CompiledRuntime::load(running, "Cpu").expect("the image loads");
    second
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&retain))),
            Some(Duration::ZERO),
        )
        .expect("the retain file is set");
    second
        .restart_with_retain_store(RestartMode::Warm)
        .expect("the warm start from the retain file");
    second.reset_task_timing(Duration::ZERO);
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(6)));
    assert_eq!(value(&second, "P.count"), Some(Value::DInt(0)));

    second
        .apply_online_change_bytes(&changed)
        .expect("the change applies");
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(6)));
    assert_eq!(value(&second, "P.added"), Some(Value::Int(7)));
    second.set_current_time(millis(10));
    second.execute_cycle().expect("the changed program runs");
    assert_eq!(value(&second, "P.count"), Some(Value::DInt(1)));
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(9)));

    second.restart(RestartMode::Cold).expect("the cold restart");
    assert_eq!(value(&second, "P.count"), Some(Value::DInt(0)));
    assert_eq!(value(&second, "P.kept"), Some(Value::DInt(0)));
    let _ = std::fs::remove_file(&retain);
}

/// Specification 11 §6.8.1 on a compiled program: a step at 10 ms with a cold restart, which
/// restarts the program at 0 ms, writes 41 into `count` and runs one scan, in which the 10 ms task is
/// due (specification 10 §4.3), so `count` becomes 42 and `kept` 2. Catches a step that does not
/// write its inputs before the scan, does not restart, or reads the watched names before the scan.
#[test]
fn a_computer_steps_a_compiled_program() {
    let mut runtime =
        CompiledRuntime::load(image(RUNNING, "step"), "Cpu").expect("the image loads");
    let request = StepRequest {
        time: millis(10),
        restart: Some(RestartMode::Cold),
        inputs: vec![(SmolStr::new("P.count"), Value::DInt(41))],
        direct_inputs: Vec::new(),
        watch: vec![SmolStr::new("P.count"), SmolStr::new("P.kept")],
        watch_direct: Vec::new(),
        run: None,
    };
    let outcome = runtime
        .step(&request, Duration::ZERO)
        .expect("the step runs");
    assert_eq!(outcome.error, None);
    assert_eq!(
        outcome.values,
        vec![
            (SmolStr::new("P.count"), Value::DInt(42)),
            (SmolStr::new("P.kept"), Value::DInt(2)),
        ]
    );
}

/// Specification 44 §8.8 (NX-14): the resource's scan thread runs a compiled program as it runs an
/// interpreted one, and a resource command reads its variables between scans. At a 10 ms cycle the
/// program runs scan after scan until its `count` reaches 5, which the test waits up to 5 s for: a
/// loaded computer may wake the cycle late, and the cycle grid bounds no delay of the operating
/// system (specification 10 §4.3). The resource's snapshot, which the HMI, OPC UA and ADS servers
/// read, then carries the compiled state (§13.3) with `count` at least 5. Catches a scan thread that
/// does not run a compiled program, a command the compiled program does not answer, or a snapshot
/// without its state.
#[test]
fn the_scan_thread_runs_a_compiled_program() {
    let mut runtime =
        CompiledRuntime::load(image(RUNNING, "thread"), "Cpu").expect("the image loads");
    runtime.restart(RestartMode::Cold).expect("the cold start");
    let runner = ResourceRunner::new(runtime, StdClock::new(), millis(10));
    let mut handle = runner.spawn("compiled").expect("the scan thread starts");
    let started = Instant::now();
    let count = loop {
        let (reply, replies) = std::sync::mpsc::channel();
        handle
            .control()
            .send_command(ResourceCommand::MeshSnapshot {
                names: vec![SmolStr::new("P.count")],
                respond_to: reply,
            })
            .expect("the command is queued");
        let values = replies
            .recv_timeout(WallDuration::from_secs(5))
            .expect("the scan thread answers");
        let count = match values.get("P.count") {
            Some(Value::DInt(count)) => *count,
            other => panic!("count: {other:?}"),
        };
        if count >= 5 || started.elapsed() >= WallDuration::from_secs(5) {
            break count;
        }
        std::thread::sleep(WallDuration::from_millis(10));
    };
    let (reply, replies) = std::sync::mpsc::channel();
    handle
        .control()
        .send_command(ResourceCommand::Snapshot { respond_to: reply })
        .expect("the command is queued");
    let snapshot = replies
        .recv_timeout(WallDuration::from_secs(5))
        .expect("the scan thread answers");
    handle.stop();
    handle.join().expect("the scan thread ends");
    let read = snapshot
        .compiled
        .as_deref()
        .and_then(|state| state.value("P.count"));
    assert!(
        matches!(read, Some(Value::DInt(read)) if read >= 5),
        "the resource snapshot carries the compiled state: {read:?}"
    );
    let elapsed = started.elapsed().as_millis();
    assert!(
        count >= 5,
        "at a 10 ms cycle, {elapsed} ms ran {count} scans, fewer than 5"
    );
}

/// The JSON of `response`.
fn json_of(response: &trust_runtime_services::protocol::ControlResponse) -> serde_json::Value {
    serde_json::to_value(response).expect("a response serializes")
}

/// The control request `kind` with `params`.
fn request(
    id: u64,
    kind: &str,
    params: Option<serde_json::Value>,
) -> trust_runtime_services::protocol::ControlRequest {
    trust_runtime_services::protocol::ControlRequest {
        id,
        r#type: kind.to_string(),
        params,
        auth: None,
        request_id: None,
    }
}

/// Specification 44 §13.2 on a computer (§8.8): a breakpoint on line 11 of `src/main.st`, `kept :=
/// kept + 2;` (zero-based line 10), holds the scan thread before that statement in the scan at 10
/// ms, when the 10 ms task is first due (specification 10 §4.3); while held, `debug.state` is paused
/// and `debug.stack`'s innermost frame stands at line 10; after `resume` the scan ends with `count` 1
/// and `kept` 2, which the debugger's published state, the one the historian samples, also holds.
/// Catches a computer whose compiled scans take no breakpoint, a hold that does not wait, a stack
/// read that does not see the held scan, a resume that does not release it, and a debugger that
/// publishes no compiled state.
#[test]
fn the_computer_debugger_holds_a_compiled_scan_at_a_breakpoint() {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!("trust-host-debug-{}", std::process::id()));
    let image = build(
        &[SourceFile::with_path("src/main.st", RUNNING)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles")
    .image;
    let _ = std::fs::remove_dir_all(&work);
    let mut runtime = CompiledRuntime::load(image, "Cpu").expect("the image loads");
    let debug = trust_runtime::debug::DebugControl::new();
    runtime.set_debug_control(debug.clone());
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(Duration::ZERO);
    let debugger = debug.compiled().expect("the compiled program's debugger");
    let io = || trust_runtime_services::live_values::io_requests::IoSnapshotRead::Missing;

    let set = json_of(&debugger.answer(
        &request(
            1,
            "breakpoints.set",
            Some(serde_json::json!({ "source": "src/main.st", "lines": [10] })),
        ),
        io(),
    ));
    assert_eq!(set["ok"], serde_json::json!(true), "{set}");

    let scan = std::thread::spawn(move || {
        runtime.set_current_time(millis(10));
        let result = runtime.execute_cycle();
        (runtime, result)
    });
    let deadline = Instant::now() + WallDuration::from_secs(10);
    loop {
        let state = json_of(&debugger.answer(&request(2, "debug.state", None), io()));
        if state["result"]["paused"] == serde_json::json!(true) {
            break;
        }
        assert!(Instant::now() < deadline, "no hold within 10 s: {state}");
        std::thread::sleep(WallDuration::from_millis(10));
    }
    let stack = json_of(&debugger.answer(&request(3, "debug.stack", None), io()));
    assert_eq!(
        stack["result"]["stack_frames"][0]["line"],
        serde_json::json!(10),
        "{stack}"
    );
    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the held scan completes");
    assert_eq!(value(&runtime, "P.count"), Some(Value::DInt(1)));
    assert_eq!(value(&runtime, "P.kept"), Some(Value::DInt(2)));
    let published = debug
        .snapshot()
        .expect("the debugger publishes the scan's state");
    let state = published.compiled.expect("a compiled program's state");
    assert_eq!(state.value("P.kept"), Some(Value::DInt(2)));
}

/// Specification 44 §13.3 on a computer (§8.8): the services read a compiled program through its
/// symbol table. After a cold start and the scan at 10 ms, which runs the 10 ms task once
/// (specification 10 §4.3), `count` is 1 and the other variables hold their declared starting
/// values. The program instance `P` lists its six variables in their declared order, each named
/// `P.<name>` as the debugger names it, with its IEC type (`STRING[20]`, the enumeration `Mode`,
/// `ARRAY[1..3] OF INT` by IEC 61131-3 Table 11, the timer `TON` as an instance), and then the
/// global `Speed`; `Mode` lists `Idle`, `Run` and `Fault`. Read through the program access the
/// HMI and Control Studio use (specification 44 §8.8, §13.3), `P.count` is 1, `P.mode` is `Run`
/// and `Speed` is 5, and the timer `P.timer` is an instance of `TON`, whose value is its identity.
/// Catches a listing that loses a program's variables or the globals, names or types them
/// otherwise, and a service that reads no compiled value.
#[test]
fn the_services_read_a_compiled_program_through_its_symbol_table() {
    let mut runtime =
        CompiledRuntime::load(image(LISTED, "listed"), "Cpu").expect("the image loads");
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(Duration::ZERO);
    runtime.set_current_time(millis(10));
    runtime.execute_cycle().expect("the scan at 10 ms");
    let state = runtime.snapshot().expect("the program keeps its image");

    let variables = state.variables();
    let listed: Vec<(&str, Option<&str>, &str, bool)> = variables
        .iter()
        .map(|variable| {
            (
                variable.name.as_str(),
                variable.program.as_deref(),
                variable.type_name.as_str(),
                variable.instance,
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            ("P.count", Some("P"), "DINT", false),
            ("P.label", Some("P"), "STRING[20]", false),
            ("P.mode", Some("P"), "Mode", false),
            ("P.history", Some("P"), "ARRAY[1..3] OF INT", false),
            ("P.timer", Some("P"), "TON", true),
            ("P.ratio", Some("P"), "REAL", false),
            ("Speed", None, "INT", false),
        ]
    );
    let mode = variables
        .iter()
        .find(|variable| variable.name == "P.mode")
        .expect("the enumeration variable");
    assert_eq!(
        state.enum_values(mode.symbol),
        vec![
            SmolStr::new("Idle"),
            SmolStr::new("Run"),
            SmolStr::new("Fault")
        ]
    );

    use trust_runtime::ProgramRead;
    assert_eq!(state.value("P.count"), Some(Value::DInt(1)));
    assert!(matches!(
        state.value("P.mode"),
        Some(Value::Enum(ref value)) if value.variant_name().as_str() == "Run"
    ));
    assert_eq!(state.value("Speed"), Some(Value::Int(5)));
    let timer = state.locate("P.timer").expect("the timer's place");
    assert!(state.is_a(&timer, "TON"));
    assert!(matches!(state.value("P.timer"), Some(Value::Instance(_))));
}

/// A program whose globals an OPC UA client connection binds: `Doubled` is twice `Speed`.
const CLIENT: &str = "
VAR_GLOBAL
    Speed : INT;
    Doubled : INT;
END_VAR

PROGRAM Main
Doubled := Speed * 2;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// An OPC UA server that answers the subscription with `Speed` 42 and `Doubled` 0, and records
/// every value written to it.
struct SampleServer {
    written: std::sync::Arc<std::sync::Mutex<Vec<(SmolStr, Value)>>>,
}

impl trust_runtime::opcua::OpcUaClientTransport for SampleServer {
    fn connect(
        &mut self,
        connection: &trust_runtime::opcua::OpcUaClientConnectionConfig,
        _sink: trust_runtime::opcua::OpcUaClientEventSink,
    ) -> Result<
        trust_runtime::opcua::OpcUaClientSessionInfo,
        trust_runtime::opcua::OpcUaClientBridgeError,
    > {
        Ok(trust_runtime::opcua::OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: Some(connection.timeout_ms),
            recovery_detail: None,
        })
    }

    fn subscribe_read_points(
        &mut self,
        points: &[trust_runtime::opcua::OpcUaClientPointConfig],
        sink: trust_runtime::opcua::OpcUaClientEventSink,
    ) -> Result<(), trust_runtime::opcua::OpcUaClientBridgeError> {
        for point in points {
            let value = if point.var == "Speed" { 42 } else { 0 };
            let taken = sink.publish_sample(trust_runtime::opcua::OpcUaClientSample {
                var: point.var.clone(),
                node_id: point.node_id.clone(),
                data_type: point.data_type,
                access: point.access,
                value: Some(Value::Int(value)),
                state: trust_runtime::opcua::OpcUaClientConnectionState::Connected,
                // Unstamped, the sample is as new as the session that takes it.
                last_seen_ms: None,
                detail: "subscribed".into(),
            });
            assert!(taken, "the client takes the sample of {}", point.var);
        }
        Ok(())
    }

    fn write_values(
        &mut self,
        values: &[(trust_runtime::opcua::OpcUaClientPointConfig, Value)],
    ) -> Result<(), trust_runtime::opcua::OpcUaClientBridgeError> {
        let mut written = self.written.lock().expect("the record");
        written.extend(
            values
                .iter()
                .map(|(point, value)| (point.var.clone(), value.clone())),
        );
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), trust_runtime::opcua::OpcUaClientBridgeError> {
        Ok(())
    }
}

/// Specification 44 §13.3 with specification 11's OPC UA client: a computer's OPC UA client binds
/// a compiled program's globals by name. The server's sample `Speed` 42 reaches the global after
/// the inputs of a scan, the program makes `Doubled` 84, twice 42, and after the scan the client
/// writes `Doubled` 84 to the server. A point naming no global is refused when the connection
/// starts. Catches a compiled program the client cannot bind, a sample that never reaches the
/// program, an output that never leaves it, and a point that binds nothing.
#[test]
fn an_opcua_client_binds_a_compiled_programs_globals() {
    use trust_runtime::opcua::{
        OpcUaClientAuthConfig, OpcUaClientConnectionConfig, OpcUaClientPointAccess,
        OpcUaClientPointConfig, OpcUaDataType, OpcUaMessageSecurityMode, OpcUaSecurityPolicy,
        OpcUaSecurityProfile,
    };
    let point = |var: &str, access| OpcUaClientPointConfig {
        var: SmolStr::new(var),
        node_id: format!("ns=2;s={var}"),
        data_type: OpcUaDataType::Int16,
        access,
    };
    let connection = |points| OpcUaClientConnectionConfig {
        name: SmolStr::new("line"),
        endpoint_url: "opc.tcp://127.0.0.1:4840/line".to_string(),
        security: OpcUaSecurityProfile {
            policy: OpcUaSecurityPolicy::None,
            mode: OpcUaMessageSecurityMode::None,
            allow_anonymous: true,
        },
        auth: OpcUaClientAuthConfig::Anonymous,
        trust_server_certificate: true,
        poll_interval_ms: 5,
        timeout_ms: 100,
        points,
    };
    let mut runtime =
        CompiledRuntime::load(image(CLIENT, "client"), "Cpu").expect("the image loads");
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(Duration::ZERO);
    let written = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let refused = runtime.start_opcua_client_connection(
        &connection(vec![point("Missing", OpcUaClientPointAccess::Read)]),
        SampleServer {
            written: std::sync::Arc::clone(&written),
        },
        WallDuration::from_millis(5),
    );
    assert!(refused.is_err(), "a point naming no global is refused");
    runtime
        .start_opcua_client_connection(
            &connection(vec![
                point("Speed", OpcUaClientPointAccess::Read),
                point("Doubled", OpcUaClientPointAccess::ReadWrite),
            ]),
            SampleServer {
                written: std::sync::Arc::clone(&written),
            },
            WallDuration::from_millis(5),
        )
        .expect("the connection starts");

    let deadline = Instant::now() + WallDuration::from_secs(10);
    let mut scan = 0;
    loop {
        scan += 1;
        runtime.set_current_time(millis(10 * scan));
        runtime.execute_cycle().expect("a scan");
        let sent = written
            .lock()
            .expect("the record")
            .contains(&(SmolStr::new("Doubled"), Value::Int(84)));
        if sent {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no write of Doubled 84 within 10 s: Speed {:?}, Doubled {:?}",
            runtime.read_global("Speed"),
            runtime.read_global("Doubled")
        );
        std::thread::sleep(WallDuration::from_millis(5));
    }
    assert_eq!(runtime.read_global("Speed"), Ok(Value::Int(42)));
}

/// A program whose variables MQTT tags bind: `doubled` is twice `level`, and `lamp` is on above 10.
const TAGGED: &str = "
PROGRAM Main
VAR
    level : INT;
    doubled : INT;
    lamp : BOOL;
END_VAR
doubled := level * 2;
lamp := level > 10;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// A driver that reads `level` 21 at the first input byte and records the output image it drives.
struct TagDriver {
    driven: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl trust_runtime::io::IoDriver for TagDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), trust_runtime::error::RuntimeError> {
        inputs[..2].copy_from_slice(&21_i16.to_le_bytes());
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), trust_runtime::error::RuntimeError> {
        *self.driven.lock().expect("the record") = outputs.to_vec();
        Ok(())
    }
}

/// Specification 44 §13.3 with specification 32's MQTT tag mappings: a compiled program's tags bind
/// its program instance's variables by name. The program binds no process image, so the read tag
/// `P.level` takes the input word at byte 0, and the write tags `P.doubled` and `P.lamp` the output
/// word at byte 0 and the bit 0 of byte 2, past the program's empty images; the driver points carry
/// those offsets and types (`i16`, `bool`). A scan at 10 ms takes `level` 21 from the driver's
/// input bytes, makes `doubled` 42 and `lamp` TRUE (21 > 10), and the driver drives the bytes 42, 0
/// and 1. A tag naming no program variable is refused. Catches a compiled program whose tags are
/// refused or bound elsewhere, an input that never reaches its variable, and an output that never
/// reaches the driver.
#[test]
fn mqtt_tags_bind_a_compiled_programs_variables() {
    let mut runtime =
        CompiledRuntime::load(image(TAGGED, "tagged"), "Cpu").expect("the image loads");
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(Duration::ZERO);
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
on_error = "warn"
mappings = [
  { tag = "P.level", topic = "plant/level", direction = "read" },
  { tag = "P.doubled", topic = "plant/doubled", direction = "write" },
  { tag = "P.lamp", topic = "plant/lamp", direction = "write" }
]
"#,
    )
    .expect("the parameters");
    let refused: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [{ tag = "P.missing", topic = "plant/missing", direction = "read" }]
"#,
    )
    .expect("the parameters");
    assert!(
        trust_runtime::io::resolve_compiled_mqtt_tag_mappings(&runtime, &refused).is_err(),
        "a tag naming no program variable is refused"
    );

    let (lowered, links) = trust_runtime::io::resolve_compiled_mqtt_tag_mappings(&runtime, &params)
        .expect("the tags resolve");
    let points = |key: &str| lowered.get(key).cloned().expect("the lowered points");
    let expected: toml::Value = toml::from_str(
        r#"
input_points = [{ topic = "plant/level", image_offset = 0, data_type = "i16" }]
output_points = [
  { topic = "plant/doubled", image_offset = 0, data_type = "i16" },
  { topic = "plant/lamp", image_offset = 2, image_bit = 0, data_type = "bool" }
]
"#,
    )
    .expect("the expected points");
    assert_eq!(points("input_points"), expected["input_points"]);
    assert_eq!(points("output_points"), expected["output_points"]);
    let linked: Vec<&str> = links.iter().map(|link| link.name.as_str()).collect();
    assert_eq!(linked, vec!["P.level", "P.doubled", "P.lamp"]);

    runtime.link_mqtt_tags(&links).expect("the tags link");
    let driven = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    runtime.add_io_driver(
        "tags",
        Box::new(TagDriver {
            driven: std::sync::Arc::clone(&driven),
        }),
    );
    runtime.set_current_time(millis(10));
    runtime.execute_cycle().expect("the scan at 10 ms");

    assert_eq!(value(&runtime, "P.level"), Some(Value::Int(21)));
    assert_eq!(value(&runtime, "P.doubled"), Some(Value::Int(42)));
    assert_eq!(value(&runtime, "P.lamp"), Some(Value::Bool(true)));
    assert_eq!(*driven.lock().expect("the record"), vec![42, 0, 1]);
}

/// A program whose scan never ends by itself.
const ENDLESS: &str = "
PROGRAM Main
VAR
    count : DINT;
END_VAR
WHILE TRUE DO
    count := count + 1;
END_WHILE;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// Specification 44 NX-10 on a computer: the project's `[resource] max_statements_per_scan` is the
/// budget of each task activation. In the scan at 10 ms the debug image counts the WHILE statement,
/// then each pass and its assignment: with a budget of 1,000, 499 passes leave one, the 500th pass
/// takes it, and its assignment ends the scan with `InfiniteLoop`, the work limit of 1,000, before
/// it stores, so `count` is 499; with 2,000 `count` is 999 and the limit 2,000. Catches a computer
/// that ignores the setting and runs its default of 1,000,000.
#[test]
fn the_statement_budget_bounds_a_compiled_scan() {
    for (budget, count) in [(1_000_u32, 499), (2_000, 999)] {
        let mut runtime = CompiledRuntime::load_with_statement_budget(
            image(ENDLESS, &format!("endless-{budget}")),
            "Cpu",
            budget,
        )
        .expect("the image loads");
        runtime.restart(RestartMode::Cold).expect("the cold start");
        runtime.reset_task_timing(Duration::ZERO);
        runtime.set_current_time(millis(10));
        let error = runtime
            .execute_cycle()
            .expect_err("the budget ends the scan");
        assert!(
            matches!(
                error,
                trust_runtime::error::RuntimeError::ExecutionLimit { limit, .. }
                    if limit == budget as usize
            ),
            "budget {budget}: {error:?}"
        );
        assert_eq!(value(&runtime, "P.count"), Some(Value::DInt(count)));
    }
}

/// A program that copies a directly addressed input global to a directly addressed output global.
const ADDRESSED: &str = "
VAR_GLOBAL
    InSignal AT %IX0.0 : BOOL;
    OutLamp AT %QX0.1 : BOOL;
END_VAR

PROGRAM Main
OutLamp := InSignal;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// Specification 44 NX-09 with IEC 61131-3 §6.5.5 (direct representation): a global declared `AT`
/// a process-image address is compiled like a program's variable `AT` one, its binding through its
/// symbol. With `%IX0.0` written TRUE, the scan at 10 ms takes it into `InSignal` at the input
/// sample, copies it to `OutLamp`, and drives `%QX0.1` TRUE at the output sample. Catches a build
/// that refuses a directly addressed global, an input that never reaches it, and an output it never
/// drives.
#[test]
fn a_directly_addressed_global_is_bound_to_its_address() {
    let mut runtime =
        CompiledRuntime::load(image(ADDRESSED, "addressed"), "Cpu").expect("the image loads");
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(Duration::ZERO);
    let input = IoAddress::parse("%IX0.0").expect("an address");
    let output = IoAddress::parse("%QX0.1").expect("an address");
    runtime
        .write_io(&input, Value::Bool(true))
        .expect("the input bit");
    runtime.set_current_time(millis(10));
    runtime.execute_cycle().expect("the scan at 10 ms");
    assert_eq!(runtime.read_global("InSignal"), Ok(Value::Bool(true)));
    assert_eq!(runtime.read_io(&output), Ok(Value::Bool(true)));
}
