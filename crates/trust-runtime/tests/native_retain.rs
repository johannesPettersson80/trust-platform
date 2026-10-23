//! The retained values of a compiled program and of today's runtime are one checkpoint
//! (specification 44 §11; specification 11 §6.7): the same program, compiled for this computer and
//! run by the kernel, and run by the runtime, writes the same STRN v3 payload byte for byte, and
//! each restores the checkpoint the other wrote.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::{SourceFile, TestHarness};
use trust_runtime::native::{build, load, Build, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot};
use trust_runtime_core::retain::capture::RetainCapture;
use trust_runtime_core::retain::codec::decode_snapshot_payload_v3;
use trust_runtime_core::retain::RetainSnapshot as Checkpoint;

/// Retained globals and program variables of every kind a checkpoint holds, a retained function
/// block instance it leaves out, a function block with a `RETAIN` variable, and a variable without
/// `RETAIN`; the program changes them in every scan.
const PROGRAM: &str = "
TYPE Mode :
(
    Idle := 0,
    Running := 1,
    Fault := -1
);
END_TYPE

TYPE Point :
STRUCT
    X : INT;
    Y : REAL;
END_STRUCT
END_TYPE

VAR_GLOBAL RETAIN
    Count : UDINT := 5;
    Ratio : LREAL := 1.5;
    Label : STRING[12] := 'start';
    State : Mode := Mode#Running;
    Wide : WSTRING[6] := \"wide\";
    Spot : Point := (X := 3, Y := 4.5);
    Fill : INT(0..100) := 50;
    Since : TIME := T#2s;
END_VAR

FUNCTION_BLOCK Calibrated
VAR_INPUT
    X : REAL;
END_VAR
VAR_OUTPUT
    Y : REAL;
END_VAR
VAR RETAIN
    Offset : REAL := 0.5;
END_VAR
Offset := Offset + 1.0;
Y := X + Offset;
END_FUNCTION_BLOCK

PROGRAM Probe
VAR RETAIN
    Total : DINT := -7;
    History : ARRAY[1..3] OF USINT := [7, 8, 9];
    Flag : BOOL := TRUE;
    Timer : TON;
END_VAR
VAR
    Speed : INT;
    Cal : Calibrated;
END_VAR
Total := Total + 1;
Count := Count + 2;
Ratio := Ratio * 2.0;
History[1] := History[1] + 1;
Spot.X := Spot.X + 1;
Fill := Fill + 1;
Flag := NOT Flag;
Since := Since + T#10ms;
Timer(IN := TRUE, PT := T#1s);
Speed := Speed + 1;
Cal(X := 1.0);
END_PROGRAM

CONFIGURATION Bench
RESOURCE Plc ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Probe;
END_RESOURCE
END_CONFIGURATION
";

const MS: i64 = 1_000_000;

/// The STRN v3 payload of the runtime's retained values.
fn runtime_payload(runtime: &mut TestHarness) -> Vec<u8> {
    let snapshot = runtime
        .runtime_mut()
        .retain_snapshot()
        .expect("the runtime captures its retained values");
    let mut payload = Vec::new();
    Checkpoint::from_values(snapshot.values().clone())
        .encode_snapshot_payload_into(&mut payload)
        .expect("the runtime's values encode");
    payload
}

/// The probe compiled for this computer, in a work folder of its own named `name`, and the device
/// it runs on.
fn compiled(name: &str) -> (Build, Device) {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-{name}-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(PROGRAM)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the probe compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
    };
    (built, device)
}

#[test]
fn a_compiled_program_and_the_runtime_write_and_read_one_checkpoint() {
    let (built, device) = compiled("retain");
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let mut runtime = TestHarness::from_source(PROGRAM).expect("the runtime builds the probe");

    // Three activations side by side.
    for scan in 1..=3i64 {
        program
            .scan(Duration::from_nanos(scan * 10 * MS), &mut NoHooks)
            .expect("the native scan");
        runtime.advance_time(Duration::from_millis(10));
        let result = runtime.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    // One payload, byte for byte: the same keys in the same order, the instance left out, the
    // values encoded alike. Catches a key, an order or an encoding of the kernel's own.
    let mut capture = RetainCapture::new();
    program
        .capture_retain(&mut capture)
        .expect("the kernel captures its retained values");
    let native = capture.staged_payload().to_vec();
    let interpreted = runtime_payload(&mut runtime);
    assert_eq!(native, interpreted);
    let checkpoint = decode_snapshot_payload_v3(&native).expect("the payload decodes");
    let keys: Vec<&str> = checkpoint.values().keys().map(|key| key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "Count",
            "Ratio",
            "Label",
            "State",
            "Wide",
            "Spot",
            "Fill",
            "Since",
            "@program/Main/Total",
            "@program/Main/History",
            "@program/Main/Flag",
        ]
    );

    // The runtime's checkpoint restores into the compiled program after a cold restart, and the
    // compiled program's into the runtime: each then writes the checkpoint it was given, with no
    // event. Catches a restore that drops, converts or misplaces a value.
    program
        .cold_start(Duration::ZERO)
        .expect("the cold restart");
    let events = program
        .warm_start_from(Duration::ZERO, &checkpoint)
        .expect("the runtime's checkpoint restores");
    assert_eq!(events, []);
    program
        .capture_retain(&mut capture)
        .expect("the kernel captures again");
    assert_eq!(capture.staged_payload(), interpreted.as_slice());

    runtime
        .restart(RestartMode::Cold)
        .expect("the runtime's cold restart");
    let mut restored = RetainSnapshot::default();
    for (key, value) in checkpoint.values() {
        restored.insert(key.clone(), value.clone());
    }
    runtime
        .runtime_mut()
        .apply_retain_snapshot(&restored)
        .expect("the kernel's checkpoint restores into the runtime");
    assert_eq!(runtime_payload(&mut runtime), native);
}

/// A `RETAIN` variable of a function block's type is a member of each instance, which the runtime
/// keeps across no warm restart and writes into no checkpoint (specification 44 §11; specification
/// 11 §6.7): after three scans `Cal.Offset` is 0.5 + 3 × 1.0 = 3.5 in both, and after a warm
/// restart it is its starting value 0.5 in both, while the program's retained `Total` keeps
/// -7 + 3 = -4. Catches a compiled program that refuses the declaration, or one that keeps the
/// member where the runtime does not.
#[test]
fn a_retained_variable_of_a_function_block_starts_again_at_a_warm_restart_as_in_the_runtime() {
    let (built, device) = compiled("retain-member");
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let mut runtime = TestHarness::from_source(PROGRAM).expect("the runtime builds the probe");
    for scan in 1..=3i64 {
        program
            .scan(Duration::from_nanos(scan * 10 * MS), &mut NoHooks)
            .expect("the native scan");
        runtime.advance_time(Duration::from_millis(10));
        let result = runtime.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }
    assert_eq!(program.value("MAIN.CAL.OFFSET"), Some(Value::Real(3.5)));
    assert_eq!(runtime.get_output("Cal.Offset"), Some(Value::Real(3.5)));

    program
        .warm_start(Duration::from_nanos(30 * MS))
        .expect("the warm restart");
    runtime
        .restart(RestartMode::Warm)
        .expect("the runtime's warm restart");
    assert_eq!(program.value("MAIN.CAL.OFFSET"), Some(Value::Real(0.5)));
    assert_eq!(runtime.get_output("Cal.Offset"), Some(Value::Real(0.5)));
    assert_eq!(program.value("MAIN.TOTAL"), Some(Value::DInt(-4)));
    assert_eq!(runtime.get_output("Total"), Some(Value::DInt(-4)));
}
