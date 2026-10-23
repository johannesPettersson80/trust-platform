//! Long temporal types at direct addresses, end to end through the harness (IEC 61131-3 Table 10;
//! spec 10 section 1.2 and spec 12 section 4.1). Each LDATE, LTOD and LDT image is one LWORD that
//! carries the signed nanoseconds of its value, the same as the LTIME image pinned in
//! `temporal_layout_direct_views.rs`. DATE, TOD and DT stay refused at direct addresses: the harness
//! and the bytecode path refuse them by name here, and the codec boundary refuses them in
//! trust-runtime-core `tests/temporal_layout_direct_long_forms.rs`.
#![cfg(feature = "full")]
use trust_runtime::{
    bytecode::BytecodeVersion,
    harness::{CompileSession, TestHarness},
    value::Value,
};

/// 1970-01-02 is one day, 86 400 s, after the LDATE epoch.
/// 01:02:03.004 is 3 723.004 s after midnight.
/// The LDT literal 1970-01-02-01:02:03.004 is one day plus that time of day after the epoch.
#[test]
fn ldate_ltod_and_ldt_images_are_long_words_of_their_nanoseconds() {
    let mut harness = TestHarness::from_source(
        r#"
VAR_GLOBAL
    day AT %QL0 : LDATE;
    clock AT %QL8 : LTOD;
    stamp AT %QL16 : LDT;
END_VAR
PROGRAM Main
    day := LDATE#1970-01-02;
    clock := LTOD#01:02:03.004;
    stamp := LDT#1970-01-02-01:02:03.004;
END_PROGRAM
"#,
    )
    .unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        harness.get_direct_output("%QL0"),
        Ok(Value::LWord(86_400_000_000_000))
    );
    assert_eq!(
        harness.get_direct_output("%QL8"),
        Ok(Value::LWord(3_723_004_000_000))
    );
    assert_eq!(
        harness.get_direct_output("%QL16"),
        Ok(Value::LWord(90_123_004_000_000))
    );
}

/// IEC 61131-3 Table 10 gives DATE, TOD and DT no fixed width, so a direct binding of any of them
/// is a compile error that names the type and the long form to use instead. The source-free
/// bytecode path refuses the same declaration.
#[test]
fn date_tod_and_dt_at_a_direct_address_are_refused_by_name() {
    for (declaration, type_name, long_form) in [
        ("d AT %QD0 : DATE", "DATE", "LDATE"),
        ("t AT %QD0 : TOD", "TOD", "LTOD"),
        ("s AT %QD0 : DT", "DT", "LDT"),
    ] {
        let source = format!("VAR_GLOBAL {declaration}; END_VAR PROGRAM Main END_PROGRAM");
        let message = match TestHarness::from_source(&source) {
            Ok(_) => panic!("{declaration} must be refused at a direct address"),
            Err(error) => error.to_string(),
        };
        assert!(
            message.contains(&format!("{type_name} has no fixed direct-address width")),
            "{message}"
        );
        assert!(message.contains(long_form), "{message}");
        assert!(
            CompileSession::from_source(&source)
                .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
                .is_err(),
            "{declaration} must be refused by the bytecode path"
        );
    }
}

/// The same refusal applies through a type alias and through an array of DATE.
#[test]
fn date_reached_through_an_alias_or_an_array_is_refused_by_name() {
    for source in [
        "TYPE Stamp : DATE; END_TYPE VAR_GLOBAL s AT %QD0 : Stamp; END_VAR PROGRAM Main END_PROGRAM",
        "VAR_GLOBAL dates AT %QB0 : ARRAY[0..1] OF DATE; END_VAR PROGRAM Main END_PROGRAM",
    ] {
        let message = match TestHarness::from_source(source) {
            Ok(_) => panic!("a DATE direct binding must be refused: {source}"),
            Err(error) => error.to_string(),
        };
        assert!(
            message.contains("DATE has no fixed direct-address width"),
            "{message}"
        );
    }
}
