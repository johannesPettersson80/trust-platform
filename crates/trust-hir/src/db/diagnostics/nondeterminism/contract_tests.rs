use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use crate::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

use super::*;

fn diagnostics_for(sources: &[&str], file: usize) -> Vec<Diagnostic> {
    let mut database = Database::new();
    for (index, source) in sources.iter().enumerate() {
        database.set_source_text(FileId(index as u32), (*source).to_owned());
    }
    database.diagnostics(FileId(file as u32)).as_ref().clone()
}

fn warnings(source: &str, code: DiagnosticCode) -> Vec<Diagnostic> {
    diagnostics_for(&[source], 0)
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Warning && diagnostic.code == code
        })
        .collect()
}

#[test]
fn every_short_and_long_time_date_family_warns() {
    let source = r#"
PROGRAM Main
VAR
    ShortTime : TIME;
    LongTime : LTIME;
    ShortDate : DATE;
    LongDate : LDATE;
    ShortTod : TOD;
    LongTod : LTOD;
    ShortDateTime : DT;
    LongDateTime : LDT;
END_VAR
END_PROGRAM
"#;
    let found = warnings(source, DiagnosticCode::NondeterministicTimeDate);
    let messages = found
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(found.len(), 8);
    for name in [
        "ShortTime",
        "LongTime",
        "ShortDate",
        "LongDate",
        "ShortTod",
        "LongTod",
        "ShortDateTime",
        "LongDateTime",
    ] {
        let expected = format!("time/date value '{name}' may introduce nondeterminism");
        assert!(
            messages.contains(&expected.as_str()),
            "missing warning for {name}: {messages:?}"
        );
    }
}

#[test]
fn aliases_and_alias_chains_resolve_to_time_family_before_classification() {
    let found = warnings(
        r#"
TYPE
    CycleTime : TIME;
    NestedCycleTime : CycleTime;
    EventDate : LDATE;
END_TYPE
PROGRAM Main
VAR
    Period : NestedCycleTime;
    Created : EventDate;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::NondeterministicTimeDate,
    );
    let messages = found
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(found.len(), 2);
    assert!(messages.iter().any(|message| message.contains("'Period'")));
    assert!(messages.iter().any(|message| message.contains("'Created'")));
}

#[test]
fn ordinary_non_time_values_do_not_receive_time_date_warning() {
    let found = warnings(
        r#"
PROGRAM Main
VAR
    Flag : BOOL;
    Count : DINT;
    Ratio : LREAL;
    Name : STRING;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::NondeterministicTimeDate,
    );

    assert!(found.is_empty());
}

#[test]
fn constants_and_type_declarations_are_not_live_time_values() {
    let found = warnings(
        r#"
TYPE CycleTime : TIME; END_TYPE
VAR_GLOBAL CONSTANT
    FixedPeriod : TIME := T#1s;
END_VAR
PROGRAM Main
VAR CONSTANT
    LocalPeriod : LTIME := LTIME#1s;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::NondeterministicTimeDate,
    );

    assert!(found.is_empty());
}

#[test]
fn parameters_function_method_results_and_properties_are_variable_like() {
    let found = warnings(
        r#"
FUNCTION ReadTime : TIME
VAR_INPUT
    Since : LTIME;
END_VAR
END_FUNCTION
CLASS Clock
METHOD ReadDate : DATE
END_METHOD
PROPERTY Stamp : DT
GET
END_GET
END_PROPERTY
END_CLASS
"#,
        DiagnosticCode::NondeterministicTimeDate,
    );
    let messages = found
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    for name in ["ReadTime", "Since", "ReadDate", "Stamp"] {
        assert!(
            messages.iter().any(|message| message.contains(name)),
            "missing variable-like declaration {name}: {messages:?}"
        );
    }
}

#[test]
fn imported_time_declaration_is_not_diagnosed_again_in_consumer_file() {
    let sources = [
        r#"
VAR_GLOBAL
    ClockValue : DT;
END_VAR
"#,
        r#"
PROGRAM Main
VAR
    Observed : DT;
END_VAR
Observed := ClockValue;
END_PROGRAM
"#,
    ];
    let owner = diagnostics_for(&sources, 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::NondeterministicTimeDate)
        .collect::<Vec<_>>();
    let consumer = diagnostics_for(&sources, 1)
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.code == DiagnosticCode::NondeterministicTimeDate
                && diagnostic.message.contains("ClockValue")
        })
        .collect::<Vec<_>>();

    assert_eq!(owner.len(), 1);
    assert!(consumer.is_empty());
}

#[test]
fn input_and_output_address_spaces_are_case_insensitive() {
    for address in [
        "%I",
        "%IX0.0",
        "%IB1",
        "%IW2",
        "%ID3",
        "%IL4",
        "%Q",
        "%QX0.0",
        "%QB1",
        "%QW2",
        "%QD3",
        "%QL4",
        "%ix0.0",
        "%qw2",
        "  %IX0.0  ",
        "\t%qW2\n",
    ] {
        assert!(is_io_address(address), "{address:?} must classify as I/O");
    }
}

#[test]
fn memory_other_and_malformed_addresses_are_not_external_io() {
    for address in [
        "", "%", "I0.0", "Q0.0", "%M", "%MX0.0", "%MB1", "%MW2", "%MD3", "%ML4", "%G0", " %  I0.0",
    ] {
        assert!(
            !is_io_address(address),
            "{address:?} must not classify as external I/O"
        );
    }
}

#[test]
fn direct_input_and_output_declarations_warn_independently() {
    let found = warnings(
        r#"
PROGRAM Main
VAR
    InputBit AT %IX0.0 : BOOL;
    InputWord AT %IW2 : WORD;
    OutputBit AT %QX0.0 : BOOL;
    OutputWord AT %QW2 : WORD;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::NondeterministicIo,
    );
    let messages = found
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(found.len(), 4);
    for (name, address) in [
        ("InputBit", "%IX0.0"),
        ("InputWord", "%IW2"),
        ("OutputBit", "%QX0.0"),
        ("OutputWord", "%QW2"),
    ] {
        assert!(messages.iter().any(|message| {
            message.contains(name)
                && message.contains(address)
                && message.contains("nondeterministic timing")
        }));
    }
}

#[test]
fn direct_memory_storage_does_not_receive_io_warning() {
    let found = warnings(
        r#"
PROGRAM Main
VAR
    MemoryBit AT %MX0.0 : BOOL;
    MemoryWord AT %MW2 : WORD;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::NondeterministicIo,
    );

    assert!(found.is_empty());
}

#[test]
fn one_declaration_can_report_time_value_and_external_io_hazards() {
    let source = r#"
PROGRAM Main
VAR
    ExternalClock AT %QD0 : DT;
END_VAR
END_PROGRAM
"#;
    let time = warnings(source, DiagnosticCode::NondeterministicTimeDate);
    let io = warnings(source, DiagnosticCode::NondeterministicIo);

    assert_eq!(time.len(), 1);
    assert_eq!(io.len(), 1);
    assert_eq!(time[0].range, io[0].range);
}

#[test]
fn warning_primary_range_and_message_select_the_declaration() {
    let source = r#"
PROGRAM Main
VAR
    SampleTime : TIME;
END_VAR
END_PROGRAM
"#;
    let warning = warnings(source, DiagnosticCode::NondeterministicTimeDate)
        .into_iter()
        .next()
        .expect("time warning");
    let expected = source.find("SampleTime").expect("declaration") as u32;

    assert_eq!(u32::from(warning.range.start()), expected);
    assert_eq!(
        warning.message,
        "time/date value 'SampleTime' may introduce nondeterminism"
    );
}
