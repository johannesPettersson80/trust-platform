//! Specification 44 §5.8 and specification 11 (`eval`): a property whose GET only returns one of its
//! block's variables is a name that reads and writes that variable, as CODESYS monitors such a
//! property (the CODESYS toolkit's B3: `ctl eval TankInst.fbPI.Kp` answered "unknown identifier"
//! while the backing field `_rKp` worked). Every expected value is worked out by hand from the program.
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;
use trust_runtime_core::native::reflect::{resolve_for_write, PathError};

/// `Kp` reads and writes `_rKp`; `Ki` only reads `_rKi`, as it has no SET; `Gain`'s GET computes, so
/// it is no name.
const PROGRAM: &str = r"
FUNCTION_BLOCK FB_PI
VAR
    _rKp : REAL := 2.5;
    _rKi : REAL := 0.5;
    _rGain : REAL := 4.0;
    Scans : INT := 0;
END_VAR
Scans := Scans + 1;
PUBLIC PROPERTY Kp : REAL
GET
    Kp := _rKp;
END_GET
SET
    _rKp := Kp;
END_SET
END_PROPERTY
PUBLIC PROPERTY Ki : REAL
GET
    Ki := _rKi;
END_GET
END_PROPERTY
PUBLIC PROPERTY Gain : REAL
GET
    Gain := _rGain * 2.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fbPI : FB_PI;
END_VAR
fbPI();
END_PROGRAM

CONFIGURATION Plant
RESOURCE Res ON PLC
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM TankInst WITH Cycle : Main;
END_RESOURCE
END_CONFIGURATION
";

fn runtime() -> CompiledRuntime {
    TestHarness::from_session(CompileSession::from_sources(vec![SourceFile::with_path(
        "src/Main.st",
        PROGRAM,
    )]))
    .unwrap_or_else(|error| panic!("the program compiles: {error}"))
    .into_runtime()
}

fn cycle(runtime: &mut CompiledRuntime) {
    let now = runtime.current_time();
    runtime.set_current_time(Duration::from_nanos(
        now.as_nanos()
            .saturating_add(Duration::from_millis(100).as_nanos()),
    ));
    runtime.execute_cycle().expect("the cycle runs");
}

/// The value at `path`, as a name read finds it; `None` for a path that names nothing.
fn value(runtime: &mut CompiledRuntime, path: &str) -> Option<Value> {
    runtime.with_program(|program| program.value(path))
}

/// Worked from the program: `Kp` reads `_rKp`'s 2.5 and `Ki` reads `_rKi`'s 0.5; `Gain` is no name.
/// Writing 3.0 to `Kp` writes `_rKp`, which `Kp` then reads; `Ki`, without a SET, is constant to a
/// write. The block's own variable counts on (`Scans` 2 after two scans), so its layout is as before.
/// Wrong behaviour caught: "unknown identifier" for a property that only returns its field, a write
/// that bypasses a property without a SET, and a property whose GET computes read as if it did not.
#[test]
fn a_property_that_returns_a_field_reads_and_writes_that_field() {
    let mut runtime = runtime();
    cycle(&mut runtime);
    assert_eq!(
        value(&mut runtime, "TankInst.fbPI.Kp"),
        Some(Value::Real(2.5))
    );
    assert_eq!(
        value(&mut runtime, "TankInst.fbPI.Ki"),
        Some(Value::Real(0.5))
    );
    assert_eq!(value(&mut runtime, "TankInst.fbPI.Gain"), None);

    runtime
        .with_program(|program| {
            let at = program.locate("TankInst.fbPI.Kp").expect("Kp is a name");
            program.write_at(&at, &Value::Real(3.0))
        })
        .expect("Kp's SET only assigns _rKp, so the write writes it");
    assert_eq!(
        value(&mut runtime, "TankInst.fbPI._rKp"),
        Some(Value::Real(3.0))
    );
    assert_eq!(
        value(&mut runtime, "TankInst.fbPI.Kp"),
        Some(Value::Real(3.0))
    );

    let read_only = runtime.with_program(|program| {
        resolve_for_write(&program.program_image(), "TankInst.fbPI.Ki", &|_| None)
    });
    assert!(
        matches!(read_only, Err(PathError::Constant(_))),
        "{read_only:?}"
    );

    cycle(&mut runtime);
    assert_eq!(
        value(&mut runtime, "TankInst.fbPI.Scans"),
        Some(Value::Int(2))
    );
}
