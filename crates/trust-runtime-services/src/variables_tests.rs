//! `variables.read` (specification 11 §6.8.1, "Reading variables"): the set's values from the
//! program's memory by their paths, the reply with the scan, its time, the state and the quality,
//! the JSON form of each elementary type, and the refusals, exactly as the specification words them.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use serde_json::{json, Value as Json};
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::memory::ProgramMemory;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{
    DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, Value,
};

use super::{read, read_set, ReadRefusal, ReadSet, MAX_PATHS};
use crate::dispatch::dispatch_value;
use crate::live_values::typed_fixture;
use crate::tests::fake_host::FakeHost;

/// The globals area of the typed fixture, which holds every variable of it.
const GLOBALS: u8 = 4;

/// The typed fixture's program memory between two scans: its image and its 28 bytes of globals.
struct FixtureMemory {
    image: Vec<u8>,
    globals: Vec<u8>,
}

impl FixtureMemory {
    fn new() -> Self {
        Self {
            image: typed_fixture::image(),
            globals: vec![0; 28],
        }
    }
}

impl ProgramMemory for FixtureMemory {
    fn image(&self) -> Image<'_> {
        Image::parse(&self.image).expect("the fixture's image reads")
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        let Some(bytes) = (id == GLOBALS)
            .then(|| self.globals.get(offset..offset + out.len()))
            .flatten()
        else {
            return false;
        };
        out.copy_from_slice(bytes);
        true
    }

    fn write_area(&mut self, id: u8, offset: usize, data: &[u8]) -> bool {
        let Some(bytes) = (id == GLOBALS)
            .then(|| self.globals.get_mut(offset..offset + data.len()))
            .flatten()
        else {
            return false;
        };
        bytes.copy_from_slice(data);
        true
    }

    fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
}

fn params(paths: &[&str]) -> Json {
    json!({ "paths": paths })
}

fn host_with(set: ReadSet) -> FakeHost {
    FakeHost {
        variables: Ok(set),
        ..FakeHost::running()
    }
}

#[test]
fn a_set_is_read_by_paths_without_case_with_each_declared_type() {
    // The fixture's globals, written as a scan would leave them: Setpoint 21.5, Level -3, Lamp TRUE,
    // Delay T#1500ms, Mode Green (1 of Color) and Name 'Pump'. Paths compare without case (IEC
    // 61131-3 §6.1.2), and each value comes with its declared type's name. Catches a read of another
    // variable, a case-sensitive lookup, and a type named otherwise than declared.
    let mut memory = FixtureMemory::new();
    memory.write_named("Setpoint", &Value::Real(21.5)).unwrap();
    memory.write_named("Level", &Value::Int(-3)).unwrap();
    memory.write_named("Lamp", &Value::Bool(true)).unwrap();
    memory
        .write_named("Delay", &Value::Time(Duration::from_millis(1500)))
        .unwrap();
    memory
        .write_named("Name", &Value::String("Pump".into()))
        .unwrap();
    // Color is an enumeration of INT: Green is 1, at the variable's two bytes (offset 16).
    memory.globals[16..18].copy_from_slice(&1i16.to_le_bytes());
    let paths: Vec<String> = ["setpoint", "LEVEL", "Lamp", "Delay", "mode", "Name"]
        .iter()
        .map(ToString::to_string)
        .collect();
    let values = read_set(&memory, &paths).expect("the set reads");
    let types: Vec<&str> = values.iter().map(|(ty, _)| ty.as_str()).collect();
    assert_eq!(types, ["REAL", "INT", "BOOL", "TIME", "Color", "STRING"]);
    assert_eq!(values[0].1, Value::Real(21.5));
    assert_eq!(values[1].1, Value::Int(-3));
    assert_eq!(values[2].1, Value::Bool(true));
    assert_eq!(values[3].1, Value::Time(Duration::from_millis(1500)));
    let Value::Enum(mode) = &values[4].1 else {
        panic!("Mode reads as an enumeration value: {:?}", values[4].1);
    };
    assert_eq!(mode.variant_name().as_str(), "Green");
    assert_eq!(values[5].1, Value::String("Pump".into()));
}

#[test]
fn a_path_that_names_nothing_refuses_the_set() {
    // Specification 11: a path that names nothing refuses the request with the path, and a refused
    // request reads nothing. Catches a set read with a gap, or with the unknown path left out.
    let memory = FixtureMemory::new();
    let paths = vec!["Level".to_string(), "Pressure".to_string()];
    assert_eq!(
        read_set(&memory, &paths),
        Err(ReadRefusal::Unknown("Pressure".to_string()))
    );
}

#[test]
fn the_reply_names_the_scan_its_time_the_state_and_the_quality() {
    // The set of scan 41 at 2.5 s; running gives `good`, any other state `stale`, the values
    // those the last completed scan left. The values come in the request's order, each with its
    // path as the request gave it. Catches a reply without the scan or its time, a quality that
    // ignores the state, and values out of order.
    let set = ReadSet {
        scan: 41,
        time_ns: 2_500_000_000,
        values: vec![
            (SmolStr::new("INT"), Value::Int(7)),
            (SmolStr::new("BOOL"), Value::Bool(false)),
        ],
    };
    let host = host_with(set);
    let reply = read(1, Some(&params(&["level", "Main.Ready"])), &host);
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(
        reply.result,
        Some(json!({
            "scan": 41,
            "time_ns": 2_500_000_000i64,
            "state": "running",
            "quality": "good",
            "values": [
                { "path": "level", "type": "INT", "value": 7 },
                { "path": "Main.Ready", "type": "BOOL", "value": false },
            ],
        }))
    );
    assert_eq!(
        host.variable_reads.borrow().as_slice(),
        [vec!["level".to_string(), "Main.Ready".to_string()]]
    );
    for state in [
        ResourceState::Paused,
        ResourceState::Stopped,
        ResourceState::Faulted,
    ] {
        host.state.set(state);
        let reply = read(2, Some(&params(&["level", "Main.Ready"])), &host);
        let result = reply.result.expect("a reply");
        assert_eq!(result["quality"], json!("stale"), "{state:?}");
        assert_eq!(
            result["state"],
            json!(alloc::format!("{state:?}").to_ascii_lowercase())
        );
    }
}

#[test]
fn each_elementary_value_has_the_json_form_of_its_type() {
    // Specification 11's forms, worked by hand: a Boolean; integers and bit strings, ULINT's
    // largest exactly; REAL 0.1 as the shortest text of the single-precision value, 0.1, not the
    // double 0.10000000149011612; LREAL as its shortest text; NaN and the infinities as strings; a
    // duration in nanoseconds; DATE, TOD and DT in milliseconds and their long forms in
    // nanoseconds; texts; an enumerator by its name. Catches a value in another unit, a REAL
    // widened to a double's text, a NaN written as an invalid JSON number, and an enumeration by
    // its number.
    let values = vec![
        (SmolStr::new("BOOL"), Value::Bool(true)),
        (SmolStr::new("SINT"), Value::SInt(-8)),
        (SmolStr::new("UDINT"), Value::UDInt(4_000_000_000)),
        (SmolStr::new("ULINT"), Value::ULInt(u64::MAX)),
        (SmolStr::new("WORD"), Value::Word(0xBEEF)),
        (SmolStr::new("REAL"), Value::Real(0.1)),
        (SmolStr::new("LREAL"), Value::LReal(-1.25e-7)),
        (SmolStr::new("LREAL"), Value::LReal(f64::NAN)),
        (SmolStr::new("REAL"), Value::Real(f32::INFINITY)),
        (SmolStr::new("LREAL"), Value::LReal(f64::NEG_INFINITY)),
        (
            SmolStr::new("TIME"),
            Value::Time(Duration::from_millis(1500)),
        ),
        (
            SmolStr::new("DATE"),
            Value::Date(DateValue::new(86_400_000)),
        ),
        (
            SmolStr::new("TIME_OF_DAY"),
            Value::Tod(TimeOfDayValue::new(3_600_000)),
        ),
        (
            SmolStr::new("DATE_AND_TIME"),
            Value::Dt(DateTimeValue::new(1_000)),
        ),
        (SmolStr::new("LDATE"), Value::LDate(LDateValue::new(5))),
        (
            SmolStr::new("LTIME_OF_DAY"),
            Value::LTod(LTimeOfDayValue::new(6)),
        ),
        (
            SmolStr::new("LDATE_AND_TIME"),
            Value::Ldt(LDateTimeValue::new(7)),
        ),
        (SmolStr::new("STRING"), Value::String("Pump".into())),
        (SmolStr::new("WSTRING"), Value::WString("Größe".into())),
        (SmolStr::new("CHAR"), Value::Char(b'A')),
        (SmolStr::new("WCHAR"), Value::WChar(0x00E9)),
        (
            SmolStr::new("Color"),
            Value::Enum(alloc::boxed::Box::new(EnumValue::from_canonical_parts(
                "Color".into(),
                "Green".into(),
                1,
            ))),
        ),
    ];
    let paths: Vec<String> = (0..values.len())
        .map(|index| alloc::format!("V{index}"))
        .collect();
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let host = host_with(ReadSet {
        scan: 1,
        time_ns: 0,
        values,
    });
    let reply = read(3, Some(&params(&path_refs)), &host);
    let result = reply.result.expect("a reply");
    let forms: Vec<Json> = result["values"]
        .as_array()
        .expect("values")
        .iter()
        .map(|value| value["value"].clone())
        .collect();
    assert_eq!(
        forms,
        [
            json!(true),
            json!(-8),
            json!(4_000_000_000u32),
            json!(u64::MAX),
            json!(0xBEEF),
            json!(0.1),
            json!(-1.25e-7),
            json!("NaN"),
            json!("Infinity"),
            json!("-Infinity"),
            json!(1_500_000_000i64),
            json!(86_400_000i64),
            json!(3_600_000i64),
            json!(1_000i64),
            json!(5),
            json!(6),
            json!(7),
            json!("Pump"),
            json!("Größe"),
            json!("A"),
            json!("é"),
            json!("Green"),
        ]
    );
    // The REAL's text is the single-precision value's shortest, as the JSON reply carries it.
    let text = serde_json::to_string(&result["values"][5]["value"]).expect("serializes");
    assert_eq!(text, "0.1");
}

#[test]
fn a_refused_read_answers_its_reason() {
    // The stable texts of specification 11. Catches a refusal reported as an empty set.
    for (refusal, text) in [
        (ReadRefusal::NoCompletedScan, "no completed scan"),
        (
            ReadRefusal::Unknown("Pressure".to_string()),
            "unknown variable: Pressure",
        ),
        (
            ReadRefusal::NotElementary("Main.Pump".to_string()),
            "not an elementary variable: Main.Pump",
        ),
    ] {
        let host = FakeHost {
            variables: Err(refusal),
            ..FakeHost::running()
        };
        let reply = read(4, Some(&params(&["x"])), &host);
        assert!(!reply.ok);
        assert_eq!(reply.error.as_deref(), Some(text));
    }
}

#[test]
fn parameters_other_than_1_to_256_paths_are_refused_before_reading() {
    // Exactly `paths`, an array of 1 to 256 strings. Catches a read of nothing, of an unbounded
    // set, and parameters taken with extra fields; a refused request asks the platform nothing.
    let host = FakeHost::running();
    let too_many: Vec<String> = (0..=MAX_PATHS)
        .map(|index| alloc::format!("V{index}"))
        .collect();
    for (params, reason) in [
        (None, "paths is required"),
        (Some(json!(["Level"])), "paths is required"),
        (
            Some(json!({ "paths": ["Level"], "scan": 1 })),
            "only paths is taken",
        ),
        (
            Some(json!({ "paths": "Level" })),
            "paths must be an array of variable paths",
        ),
        (
            Some(json!({ "paths": [] })),
            "paths must name 1 to 256 variables",
        ),
        (
            Some(json!({ "paths": too_many })),
            "paths must name 1 to 256 variables",
        ),
        (
            Some(json!({ "paths": ["Level", 3] })),
            "paths must be an array of variable paths",
        ),
    ] {
        let reply = read(5, params.as_ref(), &host);
        assert_eq!(
            reply.error.as_deref(),
            Some(alloc::format!("invalid params: {reason}").as_str())
        );
    }
    assert!(host.variable_reads.borrow().is_empty());
}

#[test]
fn a_viewer_reads_with_the_debug_surface_disabled() {
    // Specification 11 §6.9.5: `variables.read` is a Viewer read and no debug operation, so a
    // service reads a production deployment. Catches the read refused as `debug disabled`, or
    // only for a higher role.
    let host = FakeHost {
        debug_enabled: false,
        variables: Ok(ReadSet {
            scan: 9,
            time_ns: 90,
            values: vec![(SmolStr::new("INT"), Value::Int(1))],
        }),
        ..FakeHost::running()
    };
    let reply = dispatch_value(
        json!({ "id": 6, "type": "variables.read", "params": { "paths": ["Level"] } }),
        &host,
        Some("unix"),
    );
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(reply.result.expect("a reply")["scan"], json!(9));
}
