//! Storage round trip of every temporal type at the signed 64-bit boundaries (spec 10 §2.5 and
//! spec 12 §4.1: logical temporal values keep the signed 64-bit domain under both storage ABIs;
//! only SIZEOF and physical widths change). Values are written, copied by an assignment in the
//! cycle, and read back exactly.
#![cfg(feature = "full")]
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::CompileSession,
    value::{
        DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
        TimeOfDayValue, Value,
    },
    Runtime,
};

const SOURCE: &str = r#"
VAR_GLOBAL
    time_in : TIME; time_out : TIME;
    ltime_in : LTIME; ltime_out : LTIME;
    date_in : DATE; date_out : DATE;
    ldate_in : LDATE; ldate_out : LDATE;
    tod_in : TOD; tod_out : TOD;
    ltod_in : LTOD; ltod_out : LTOD;
    dt_in : DT; dt_out : DT;
    ldt_in : LDT; ldt_out : LDT;
END_VAR
PROGRAM Main
    time_out := time_in;
    ltime_out := ltime_in;
    date_out := date_in;
    ldate_out := ldate_in;
    tod_out := tod_in;
    ltod_out := ltod_in;
    dt_out := dt_in;
    ldt_out := ldt_in;
END_PROGRAM
"#;

/// Input and output globals of each temporal type, in the order of `typed`.
const NAMES: [(&str, &str); 8] = [
    ("time_in", "time_out"),
    ("ltime_in", "ltime_out"),
    ("date_in", "date_out"),
    ("ldate_in", "ldate_out"),
    ("tod_in", "tod_out"),
    ("ltod_in", "ltod_out"),
    ("dt_in", "dt_out"),
    ("ldt_in", "ldt_out"),
];

fn typed(kind: usize, value: i64) -> Value {
    match kind {
        0 => Value::Time(Duration::from_nanos(value)),
        1 => Value::LTime(Duration::from_nanos(value)),
        2 => Value::Date(DateValue::new(value)),
        3 => Value::LDate(LDateValue::new(value)),
        4 => Value::Tod(TimeOfDayValue::new(value)),
        5 => Value::LTod(LTimeOfDayValue::new(value)),
        6 => Value::Dt(DateTimeValue::new(value)),
        _ => Value::Ldt(LDateTimeValue::new(value)),
    }
}

fn install(source: &str, version: BytecodeVersion) -> Runtime {
    let session = CompileSession::from_source(source);
    let module = session.build_bytecode_module_for_version(version).unwrap();
    if version == BytecodeVersion::LEGACY {
        session.build_runtime_from_bytecode(&module, None).unwrap()
    } else {
        Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap()
    }
}

#[test]
fn every_temporal_type_keeps_the_extreme_64_bit_values_through_storage_in_both_abis() {
    for version in [BytecodeVersion::LEGACY, BytecodeVersion::SOURCE_FREE] {
        let mut runtime = install(SOURCE, version);
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            for (kind, (input, _)) in NAMES.iter().enumerate() {
                runtime.write_global(input, typed(kind, value)).unwrap();
            }
            runtime.execute_cycle().unwrap();
            for (kind, (_, output)) in NAMES.iter().enumerate() {
                assert_eq!(
                    runtime.storage().get_global(output),
                    Some(&typed(kind, value)),
                    "{version:?} {output} {value}"
                );
            }
        }
    }
}
