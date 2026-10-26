//! The retained values of a compiled program run by the kernel and by the runtime that hosts it are
//! one checkpoint (specification 44 §11; specification 11 §6.7): the same program, compiled for
//! this computer and run by the kernel, and run by the runtime, writes the same STRN v3 payload byte
//! for byte, and each restores the checkpoint the other wrote.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{SourceFile, TestHarness};
use trust_runtime::native::{build, load, Build, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::retain::RetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot};
use trust_runtime_core::retain::capture::RetainCapture;
use trust_runtime_core::retain::codec::decode_snapshot_payload_v3;
use trust_runtime_core::retain::RetainSnapshot as Checkpoint;

/// Retained globals and program variables of every kind a checkpoint holds, a timer declared
/// `RETAIN`, a function block with a `RETAIN` variable, and a variable without `RETAIN`; the
/// program changes them in every scan.
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

/// A retain file that loads `checkpoint` and keeps the STRN v3 payload the runtime writes.
struct PayloadStore {
    checkpoint: RetainSnapshot,
    payload: Arc<Mutex<Vec<u8>>>,
}

impl RetainStore for PayloadStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(self.checkpoint.clone())
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        panic!("a compiled program's capture writes its payload, not a snapshot")
    }

    fn store_payload(&self, payload: &[u8]) -> Result<(), RuntimeError> {
        *self.payload.lock().expect("the payload lock") = payload.to_vec();
        Ok(())
    }
}

/// The STRN v3 payload of the runtime's retained values, as the runtime saves them into its retain
/// file.
fn runtime_payload(runtime: &mut TestHarness) -> Vec<u8> {
    let payload = Arc::new(Mutex::new(Vec::new()));
    runtime
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(PayloadStore {
                checkpoint: RetainSnapshot::default(),
                payload: Arc::clone(&payload),
            })),
            None,
        )
        .expect("the retain file is set");
    runtime
        .runtime_mut()
        .save_retain_store()
        .expect("the runtime saves its retained values");
    let payload = payload.lock().expect("the payload lock").clone();
    assert!(!payload.is_empty(), "the runtime wrote its retained values");
    payload
}

/// Restores `checkpoint` into the runtime: a warm restart from a retain file that holds it.
fn restore(runtime: &mut TestHarness, checkpoint: RetainSnapshot) {
    runtime
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(PayloadStore {
                checkpoint,
                payload: Arc::default(),
            })),
            None,
        )
        .expect("the retain file is set");
    runtime
        .restart_with_retain(RestartMode::Warm)
        .expect("the kernel's checkpoint restores into the runtime");
}

/// The probe compiled for this computer, in a work folder of its own named `name`, and the device
/// it runs on.
fn compiled(name: &str) -> (Build, Device) {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
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
        trusted_keys: &[],
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

    // One payload, byte for byte: the same keys in the same order, the values encoded alike
    // (specification 11 §6.7): the globals, the program's retained variables, and then the
    // variables of its instances, the timer's inputs and outputs (it is declared `RETAIN`) and the
    // `RETAIN` variable of `Cal`. Catches a key, an order or an encoding of the kernel's own.
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
            "@program-instance/Main/Timer/IN",
            "@program-instance/Main/Timer/PT",
            "@program-instance/Main/Timer/Q",
            "@program-instance/Main/Timer/ET",
            "@program-instance/Main/Cal/Offset",
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
    restore(&mut runtime, restored);
    assert_eq!(runtime_payload(&mut runtime), native);
}

/// A `RETAIN` variable of a function block's type is retained in each instance (IEC §6.5.6;
/// specification 11 §6.7; specification 44 §11): after three scans `Cal.Offset` is
/// 0.5 + 3 × 1.0 = 3.5 in both, and after a warm restart it is still 3.5 in both, as the
/// program's retained `Total` keeps -7 + 3 = -4. Catches a member that starts again at a warm
/// restart, which both engines did before 23 October 2026.
#[test]
fn a_retained_variable_of_a_function_block_keeps_its_value_at_a_warm_restart() {
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
    assert_eq!(program.value("MAIN.CAL.OFFSET"), Some(Value::Real(3.5)));
    assert_eq!(runtime.get_output("Cal.Offset"), Some(Value::Real(3.5)));
    assert_eq!(program.value("MAIN.TOTAL"), Some(Value::DInt(-4)));
    assert_eq!(runtime.get_output("Total"), Some(Value::DInt(-4)));
}

/// Every rule of IEC §6.5.6.2 for the variables of instances: a `RETAIN` variable and a `RETAIN`
/// input of the type are retained in every instance; an unqualified one only in an instance
/// declared `RETAIN`; a `NON_RETAIN` one never; an instance that a variable not kept as a whole
/// holds follows its own type, in an array too, a derived type's base variables with its own, a
/// global's instance as a program's; and a configured program's `RETAIN` qualifies its unqualified
/// variables but not its instances.
const INSTANCES: &str = "
FUNCTION_BLOCK Counter
VAR_INPUT RETAIN
    Preset : DINT;
END_VAR
VAR RETAIN
    Hours : DINT;
END_VAR
VAR
    Scratch : DINT;
END_VAR
VAR NON_RETAIN
    Session : DINT;
END_VAR
Hours := Hours + 1;
Scratch := Scratch + 1;
Session := Session + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Line
VAR
    Pump : Counter;
END_VAR
Pump(Preset := 2);
END_FUNCTION_BLOCK

FUNCTION_BLOCK Extended EXTENDS Counter
VAR RETAIN
    Extra : DINT;
END_VAR
Extra := Extra + 10;
Hours := Hours + 100;
END_FUNCTION_BLOCK

VAR_GLOBAL
    Shared : Counter;
END_VAR

PROGRAM Plant
VAR RETAIN
    Plain : DINT;
END_VAR
VAR
    One : Counter;
    Many : ARRAY[1..2] OF Counter;
    Kept : Line;
    Derived : Extended;
END_VAR
VAR RETAIN
    Whole : Counter;
END_VAR
VAR_EXTERNAL
    Shared : Counter;
END_VAR
Plain := Plain + 1;
One(Preset := 5);
Many[1](Preset := 6);
Many[2](Preset := 7);
Kept();
Derived();
Whole(Preset := 9);
Shared(Preset := 4);
END_PROGRAM

CONFIGURATION Bench
RESOURCE Plc ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Plant;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn the_variables_of_instances_are_retained_as_iec_6_5_6_says_in_both_engines() {
    // IEC §6.5.6.2 and specification 11 §6.7, worked by hand for three scans of `Plant`. Each
    // `Counter` that is called counts `Hours`, `Scratch` and `Session` to 3; `Derived` runs only
    // its own body, `Extra` 3 × 10 = 30 and `Hours` 3 × 100 = 300, and its inherited `Preset` is
    // never given, so 0. The checkpoint holds, in this
    // order: the program's `RETAIN` `Plain`; the global instance's `RETAIN` input and variable;
    // then, in declaration order, `One`'s, each element of `Many`'s, `Kept.Pump`'s (an instance
    // follows its own type), `Derived`'s own `Extra` and then its base's, and `Whole`'s, whose
    // unqualified `Scratch` is retained too
    // because `Whole` is declared `RETAIN`, but not its `NON_RETAIN` `Session`. Catches a member
    // left out, an unqualified member retained without a `RETAIN` holder, a `NON_RETAIN` one
    // retained, a holder's qualifier passed into a nested instance, a base's variables lost, and
    // either side writing another key, order or encoding than the other.
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-instances-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(INSTANCES)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the instances compile");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
        trusted_keys: &[],
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let mut runtime = TestHarness::from_source(INSTANCES).expect("the runtime builds them");
    for scan in 1..=3i64 {
        program
            .scan(Duration::from_nanos(scan * 10 * MS), &mut NoHooks)
            .expect("the native scan");
        runtime.advance_time(Duration::from_millis(10));
        let result = runtime.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }
    let mut capture = RetainCapture::new();
    program
        .capture_retain(&mut capture)
        .expect("the kernel captures");
    let native = capture.staged_payload().to_vec();
    let interpreted = runtime_payload(&mut runtime);
    let checkpoint = decode_snapshot_payload_v3(&native).expect("the payload decodes");
    let interpreted_checkpoint =
        decode_snapshot_payload_v3(&interpreted).expect("the runtime's payload decodes");
    let expected = [
        ("@program/Main/Plain", 3),
        ("@global-instance/Shared/Preset", 4),
        ("@global-instance/Shared/Hours", 3),
        ("@program-instance/Main/One/Preset", 5),
        ("@program-instance/Main/One/Hours", 3),
        ("@program-instance/Main/Many[1]/Preset", 6),
        ("@program-instance/Main/Many[1]/Hours", 3),
        ("@program-instance/Main/Many[2]/Preset", 7),
        ("@program-instance/Main/Many[2]/Hours", 3),
        ("@program-instance/Main/Kept/Pump.Preset", 2),
        ("@program-instance/Main/Kept/Pump.Hours", 3),
        ("@program-instance/Main/Derived/Extra", 30),
        ("@program-instance/Main/Derived/Preset", 0),
        ("@program-instance/Main/Derived/Hours", 300),
        ("@program-instance/Main/Whole/Preset", 9),
        ("@program-instance/Main/Whole/Hours", 3),
        ("@program-instance/Main/Whole/Scratch", 3),
    ];
    let entries = |checkpoint: &Checkpoint| -> Vec<(String, Value)> {
        checkpoint
            .values()
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect()
    };
    let wanted: Vec<(String, Value)> = expected
        .iter()
        .map(|(key, value)| ((*key).to_string(), Value::DInt(*value)))
        .collect();
    assert_eq!(entries(&checkpoint), wanted);
    // The same keys, in the same order, with the same values, from the runtime.
    assert_eq!(entries(&interpreted_checkpoint), wanted);

    // A warm restart keeps exactly these and starts the rest again, on both sides.
    program
        .warm_start(Duration::from_nanos(30 * MS))
        .expect("the warm restart");
    runtime
        .restart(RestartMode::Warm)
        .expect("the runtime's warm restart");
    for (native_path, runtime_path, value) in [
        ("MAIN.PLAIN", "Plain", 3),
        ("MAIN.ONE.HOURS", "One.Hours", 3),
        ("MAIN.ONE.SCRATCH", "One.Scratch", 0),
        ("MAIN.ONE.SESSION", "One.Session", 0),
        ("MAIN.KEPT.PUMP.HOURS", "Kept.Pump.Hours", 3),
        ("MAIN.KEPT.PUMP.SCRATCH", "Kept.Pump.Scratch", 0),
        ("MAIN.DERIVED.EXTRA", "Derived.Extra", 30),
        ("MAIN.DERIVED.HOURS", "Derived.Hours", 300),
        ("MAIN.WHOLE.SCRATCH", "Whole.Scratch", 3),
        ("MAIN.WHOLE.SESSION", "Whole.Session", 0),
    ] {
        assert_eq!(
            program.value(native_path),
            Some(Value::DInt(value)),
            "{native_path}"
        );
        assert_eq!(
            runtime.get_output(runtime_path),
            Some(Value::DInt(value)),
            "{runtime_path}"
        );
    }
    // The elements of `Many` kept theirs too: the checkpoint is the one before the restart.
    program
        .capture_retain(&mut capture)
        .expect("the kernel captures again");
    assert_eq!(capture.staged_payload(), native.as_slice());
    assert_eq!(runtime_payload(&mut runtime), interpreted);

    // Each side's checkpoint restores into the other after a cold restart, with no event, and
    // is written back byte for byte.
    program
        .cold_start(Duration::ZERO)
        .expect("the cold restart");
    assert_eq!(program.value("MAIN.ONE.HOURS"), Some(Value::DInt(0)));
    let events = program
        .warm_start_from(Duration::ZERO, &interpreted_checkpoint)
        .expect("the runtime's checkpoint restores");
    assert_eq!(events, []);
    program
        .capture_retain(&mut capture)
        .expect("the kernel captures a third time");
    assert_eq!(capture.staged_payload(), interpreted.as_slice());
    runtime
        .restart(RestartMode::Cold)
        .expect("the runtime's cold restart");
    assert_eq!(runtime.get_output("One.Hours"), Some(Value::DInt(0)));
    let mut restored = RetainSnapshot::default();
    for (key, value) in checkpoint.values() {
        restored.insert(key.clone(), value.clone());
    }
    restore(&mut runtime, restored);
    assert_eq!(runtime_payload(&mut runtime), native);
}

/// A program instance declared `RETAIN` in its configuration, of a type with an instance.
const RETAINED_PROGRAM: &str = "
FUNCTION_BLOCK Counter
VAR RETAIN
    Hours : DINT;
END_VAR
VAR
    Scratch : DINT;
END_VAR
Hours := Hours + 1;
Scratch := Scratch + 1;
END_FUNCTION_BLOCK

PROGRAM Plant
VAR
    Plain : DINT;
    Pump : Counter;
END_VAR
VAR NON_RETAIN
    Session : DINT;
END_VAR
Plain := Plain + 1;
Session := Session + 1;
Pump();
END_PROGRAM

CONFIGURATION Bench
RESOURCE Plc ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM RETAIN Main WITH Fast : Plant;
END_RESOURCE
END_CONFIGURATION
";

/// `PROGRAM RETAIN` qualifies the program's unqualified variables, and an instance the program
/// holds follows its own type's declarations (specification 11 §6.7), worked by hand for three
/// scans of the compiled program: the checkpoint holds `Plain`, 3, then `Pump.Hours`, 3, which
/// `Counter` declares `RETAIN`; not the `NON_RETAIN` `Session` and not `Pump.Scratch`. A warm
/// restart keeps those two and starts `Session` and `Pump.Scratch` again. Catches the retained
/// program instance refused (NX-09, as before 24 October 2026), the program's qualifier passed
/// into the instance (`Pump.Scratch` retained) and an unqualified variable left out.
#[test]
fn a_retained_program_instance_retains_its_variables_but_not_into_its_instances() {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!(
        "trust-native-retained-program-{}",
        std::process::id()
    ));
    let built = build(
        &[SourceFile::new(RETAINED_PROGRAM)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the retained program instance compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
        trusted_keys: &[],
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for scan in 1..=3i64 {
        program
            .scan(Duration::from_nanos(scan * 10 * MS), &mut NoHooks)
            .expect("the scan");
    }
    let mut capture = RetainCapture::new();
    program
        .capture_retain(&mut capture)
        .expect("the kernel captures");
    let checkpoint =
        decode_snapshot_payload_v3(capture.staged_payload()).expect("the payload decodes");
    let entries: Vec<(&str, Value)> = checkpoint
        .values()
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect();
    assert_eq!(
        entries,
        vec![
            ("@program/Main/Plain", Value::DInt(3)),
            ("@program-instance/Main/Pump/Hours", Value::DInt(3)),
        ]
    );
    program
        .warm_start(Duration::from_nanos(30 * MS))
        .expect("the warm restart");
    for (path, value) in [
        ("MAIN.PLAIN", 3),
        ("MAIN.SESSION", 0),
        ("MAIN.PUMP.HOURS", 3),
        ("MAIN.PUMP.SCRATCH", 0),
    ] {
        assert_eq!(program.value(path), Some(Value::DInt(value)), "{path}");
    }
}
