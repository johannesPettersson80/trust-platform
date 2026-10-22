//! Types of a compiled program that lie alike but are different types (specification 44 §6.2,
//! §6.5): arrays of one Rust storage keep their own bounds and their own element types.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, load, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::value::{ArrayValue, Duration, EnumValue, Value};

/// `low` and `high` both lie as three `INT`s, `first` and `second` as two `INT`s each; each writes
/// its last element by its own bounds.
const ARRAYS: &str = "
TYPE Shape : (Round, Square);
END_TYPE
TYPE Colour : (Red, Green);
END_TYPE

PROGRAM Main
VAR
    low : ARRAY[0..2] OF INT;
    high : ARRAY[1..3] OF INT;
    first : ARRAY[0..1] OF Shape;
    second : ARRAY[0..1] OF Colour;
END_VAR
low[2] := 5;
high[3] := 7;
first[1] := Shape#Square;
second[1] := Colour#Green;
END_PROGRAM
";

fn array(elements: Vec<Value>, lower: i64) -> Value {
    let upper = lower + i64::try_from(elements.len()).unwrap() - 1;
    Value::Array(Box::new(ArrayValue::from_canonical_parts(
        elements,
        vec![(lower, upper)],
    )))
}

fn variant(ty: &str, name: &str, value: i64) -> Value {
    Value::Enum(Box::new(EnumValue::from_canonical_parts(
        ty.into(),
        name.into(),
        value,
    )))
}

/// Each array keeps its own bounds and element type (IEC §6.4.4.5): `high[3]` is the last element
/// of `high`, and `second[1]` a `Colour`. The build gave two arrays of one storage one type, so
/// `high[3]` was checked against `low`'s bounds 0..2 and faulted, and `Colour#Green` was looked up
/// among `Shape`'s values and refused (22 October 2026, the PLCopen motion library).
#[test]
fn arrays_of_one_storage_keep_their_bounds_and_elements() {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-types-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(ARRAYS)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the arrays compile");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(
        program.value("MAIN.LOW"),
        Some(array(vec![Value::Int(0), Value::Int(0), Value::Int(5)], 0))
    );
    assert_eq!(
        program.value("MAIN.HIGH"),
        Some(array(vec![Value::Int(0), Value::Int(0), Value::Int(7)], 1))
    );
    assert_eq!(
        program.value("MAIN.FIRST"),
        Some(array(
            vec![variant("Shape", "Round", 0), variant("Shape", "Square", 1)],
            0
        ))
    );
    assert_eq!(
        program.value("MAIN.SECOND"),
        Some(array(
            vec![variant("Colour", "Red", 0), variant("Colour", "Green", 1)],
            0
        ))
    );
}
