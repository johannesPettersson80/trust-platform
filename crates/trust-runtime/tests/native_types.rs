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
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
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
        trusted_keys: &[],
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

/// Bit strings given integer literals: initial values, an untyped literal assigned, a typed one.
const WORDS: &str = "
PROGRAM Main
VAR
    d : DWORD := 4294967295;
    l : LWORD := 16#FFFF_FFFF_FFFF_FFFF;
    b : BYTE := 255;
    t : LWORD;
    w : WORD;
END_VAR
t := LWORD#16#FFFF_FFFF_FFFF_FFFE;
w := 65535;
END_PROGRAM
";

/// An integer literal for a bit string takes the unsigned range of the bit string's width (IEC
/// §6.3.2, Table 5): `DWORD := 4294967295`, `LWORD := 16#FFFF_FFFF_FFFF_FFFF`, `BYTE := 255`,
/// `WORD := 65535` and `LWORD#16#FFFF_FFFF_FFFF_FFFE` hold their values, in both engines, and
/// `BYTE := 256` is refused. Catches a bit string given the `DINT` range ("integer literal out of
/// range" for the `DWORD`) and a typed `LWORD` held as an `LINT`, as before 23 October 2026.
#[test]
fn a_bit_string_takes_an_integer_literal_of_its_width() {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-words-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(WORDS)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the bit strings compile");
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
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    let mut runtime =
        trust_runtime::harness::TestHarness::from_source(WORDS).expect("the runtime builds them");
    let result = runtime.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    for (name, expected) in [
        ("d", Value::DWord(4_294_967_295)),
        ("l", Value::LWord(u64::MAX)),
        ("b", Value::Byte(255)),
        ("t", Value::LWord(u64::MAX - 1)),
        ("w", Value::Word(65_535)),
    ] {
        assert_eq!(
            program.value(&format!("MAIN.{}", name.to_ascii_uppercase())),
            Some(expected.clone()),
            "{name}"
        );
        assert_eq!(runtime.get_output(name), Some(expected), "{name}");
    }
    let refused = trust_runtime::harness::TestHarness::from_source(
        "PROGRAM Main\nVAR\n    b : BYTE := 256;\nEND_VAR\nEND_PROGRAM\n",
    );
    assert!(refused.is_err(), "BYTE := 256 is out of range");
}
