//! An online change of a compiled program (specification 44 §12; specification 11 §6.7.1): the
//! running program's values carried by name and type into the program of a changed image, which
//! then runs on with them.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::SourceFile;
use trust_runtime::native::{
    build, load, Device, HostCode, NativeProgram, Options, Target, Toolchain,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime_core::vm::CarriedValueEvent;

/// The running program. Three scans of it leave `c.count` 6, `total` 3, `speed` 30, `big` 70,000,
/// `mode` `Run` (number 1), `levels` [3, 6, 9, 12, 15], `name` 'pump', `gone` 9 and `i` 6.
const RUNNING: &str = "
TYPE Mode : (Idle, Run, Stop); END_TYPE

FUNCTION_BLOCK Counter
VAR_INPUT
    increment : INT;
END_VAR
VAR_OUTPUT
    count : DINT;
END_VAR
count := count + increment;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
    total : INT;
    speed : INT;
    big : DINT;
    mode : Mode := Mode#Run;
    levels : ARRAY[1..5] OF INT;
    name : STRING[10] := 'pump';
    gone : INT;
    i : INT;
END_VAR
c(increment := 2);
total := total + 1;
speed := total * 10;
big := 70000;
FOR i := 1 TO 5 DO
    levels[i] := i * total;
END_FOR;
gone := 9;
END_PROGRAM
";

/// The changed program: `Counter` gains `limit`, `speed` becomes a `DINT`, `big` an `INT`, `Mode`
/// renumbers its values, `levels` moves to `3..8`, `name` shrinks to five characters, `added` is
/// new and `gone` is removed.
const CHANGED: &str = "
TYPE Mode : (Stop, Idle, Run, Fault); END_TYPE

FUNCTION_BLOCK Counter
VAR_INPUT
    increment : INT;
END_VAR
VAR_OUTPUT
    count : DINT;
END_VAR
VAR
    limit : INT := 100;
END_VAR
count := count + increment;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
    total : INT;
    speed : DINT;
    big : INT;
    mode : Mode := Mode#Idle;
    levels : ARRAY[3..8] OF INT;
    name : STRING[5] := 'x';
    added : INT := 7;
    i : INT;
END_VAR
c(increment := 2);
total := total + 1;
speed := total * 10;
FOR i := 3 TO 8 DO
    levels[i] := i * total;
END_FOR;
END_PROGRAM
";

fn program(source: &str, name: &str) -> NativeProgram<'static, HostCode> {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-{name}-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 10_000,
    };
    let image: &'static [u8] = Box::leak(built.image.into_boxed_slice());
    load(image, &device).expect("the image loads")
}

/// The events as (kind, name in capitals).
fn events(list: &[CarriedValueEvent]) -> Vec<(&'static str, String)> {
    list.iter()
        .map(|event| match event {
            CarriedValueEvent::New { name } => ("new", name.to_ascii_uppercase()),
            CarriedValueEvent::Converted { name } => ("converted", name.to_ascii_uppercase()),
            CarriedValueEvent::NotKept { name } => ("not kept", name.to_ascii_uppercase()),
            CarriedValueEvent::Cleared { name } => ("cleared", name.to_ascii_uppercase()),
            CarriedValueEvent::More { count } => ("more", count.to_string()),
        })
        .collect()
}

/// Specification 44 §12.2 with specification 11 §6.7.1, worked by hand from the two sources after
/// three scans of the running program: `total` 3 and `i` 6 are copied; `c.count` 6 is kept in the
/// changed `Counter` by its member's name and the new `c.limit` takes its starting value 100;
/// `speed` 30 converts to the `DINT` 30; `big` 70,000 does not fit an `INT` and takes 0; `mode`
/// keeps `Run` by name, now number 2; `levels` keeps indices 3 to 5 (9, 12, 15) and takes 0 at 6 to
/// 8; `name` 'pump' fits five characters; `added` is new and takes 7. One scan of the changed
/// program then counts on from the carried values: `c.count` 8, `total` 4, `speed` 40 and `levels`
/// [12, 16, 20, 24, 28, 32]. Catches values the change does not carry, a member matched by its
/// place rather than its name, a conversion that cuts or wraps, an enumeration carried by number,
/// array elements shifted by their position, and a program that does not run on from the carried
/// values.
#[test]
fn an_online_change_carries_the_running_values_by_name_and_type() {
    let mut running = program(RUNNING, "change-running");
    running.cold_start(Duration::ZERO).expect("the cold start");
    for scan in 0..3 {
        running
            .scan(
                Duration::from_millis(10 * scan),
                &mut trust_runtime::native::NoHooks,
            )
            .expect("the scan runs");
    }
    assert_eq!(running.value("MAIN.TOTAL"), Some(Value::Int(3)));

    let mut changed = program(CHANGED, "change-new");
    let carried = changed
        .change_from(&running, Duration::from_millis(20))
        .expect("the change applies");
    let value = |name: &str| changed.value(name);
    assert_eq!(value("MAIN.TOTAL"), Some(Value::Int(3)));
    assert_eq!(value("MAIN.I"), Some(Value::Int(6)));
    assert_eq!(value("MAIN.C.COUNT"), Some(Value::DInt(6)));
    assert_eq!(value("MAIN.C.LIMIT"), Some(Value::Int(100)));
    assert_eq!(value("MAIN.SPEED"), Some(Value::DInt(30)));
    assert_eq!(value("MAIN.BIG"), Some(Value::Int(0)));
    assert_eq!(value("MAIN.ADDED"), Some(Value::Int(7)));
    assert_eq!(value("MAIN.NAME"), Some(Value::String("pump".into())));
    match value("MAIN.MODE") {
        Some(Value::Enum(mode)) => {
            assert_eq!(mode.variant_name(), "Run");
            assert_eq!(mode.numeric_value(), 2);
        }
        other => panic!("mode: {other:?}"),
    }
    match value("MAIN.LEVELS") {
        Some(Value::Array(levels)) => assert_eq!(
            levels.elements(),
            [9, 12, 15, 0, 0, 0].map(Value::Int).as_slice()
        ),
        other => panic!("levels: {other:?}"),
    }
    let recorded = events(&carried);
    for expected in [
        ("converted", "MAIN.SPEED"),
        ("not kept", "MAIN.BIG"),
        ("converted", "MAIN.MODE"),
        ("converted", "MAIN.NAME"),
        ("new", "MAIN.ADDED"),
    ] {
        assert!(
            recorded
                .iter()
                .any(|(kind, name)| *kind == expected.0 && name == expected.1),
            "{expected:?} in {recorded:?}"
        );
    }
    assert!(
        !recorded
            .iter()
            .any(|(_, name)| name == "MAIN.TOTAL" || name == "MAIN.I"),
        "a copied value records no event: {recorded:?}"
    );

    changed
        .scan(
            Duration::from_millis(30),
            &mut trust_runtime::native::NoHooks,
        )
        .expect("the changed program runs");
    assert_eq!(changed.value("MAIN.C.COUNT"), Some(Value::DInt(8)));
    assert_eq!(changed.value("MAIN.TOTAL"), Some(Value::Int(4)));
    assert_eq!(changed.value("MAIN.SPEED"), Some(Value::DInt(40)));
    match changed.value("MAIN.LEVELS") {
        Some(Value::Array(levels)) => assert_eq!(
            levels.elements(),
            [12, 16, 20, 24, 28, 32].map(Value::Int).as_slice()
        ),
        other => panic!("levels: {other:?}"),
    }
}
