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

fn time_warnings(source: &str) -> Vec<Diagnostic> {
    warnings(source, DiagnosticCode::NondeterministicTimeDate)
}

fn time_warnings_in(sources: &[&str], file: usize) -> Vec<Diagnostic> {
    diagnostics_for(sources, file)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::NondeterministicTimeDate)
        .collect()
}

/// Wrong behaviour this pins: the pre-#250 rule warned on every TIME/DATE-typed
/// declaration. Specification 28 section 5: a declaration is storage and never
/// warns, whatever TIME-family type it holds.
#[test]
fn time_date_family_declarations_do_not_warn() {
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
    assert!(time_warnings(source).is_empty());
}

/// Wrong behaviour this pins: alias resolution reintroducing the declaration
/// trigger. Specification 28 section 5: aliases of the TIME family are storage
/// too and do not warn.
#[test]
fn aliased_time_declarations_do_not_warn() {
    let found = time_warnings(
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
    );

    assert!(found.is_empty());
}

#[test]
fn ordinary_non_time_values_do_not_receive_time_date_warning() {
    let found = time_warnings(
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
    );

    assert!(found.is_empty());
}

#[test]
fn constants_and_type_declarations_are_not_live_time_values() {
    let found = time_warnings(
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
    );

    assert!(found.is_empty());
}

/// Wrong behaviour this pins: parameters, results and properties of the TIME
/// family warning as declarations. Specification 28 section 5: a declaration
/// never warns, whatever variable-like shape it has.
#[test]
fn variable_like_time_declarations_never_warn() {
    let found = time_warnings(
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
    );

    assert!(found.is_empty());
}

/// Wrong behaviour this pins: a DT global warning again in every consuming
/// file. Specification 28 section 5: declarations never warn, in their owner
/// file or anywhere else.
#[test]
fn imported_time_declaration_is_not_diagnosed_in_consumer_file() {
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

    assert!(time_warnings_in(&sources, 0).is_empty());
    assert!(time_warnings_in(&sources, 1).is_empty());
}

/// Specification 28 section 5: `CURRENT_DT` is the clock source, so its call
/// warns and the message names the source. The range selects the call.
#[test]
fn a_direct_current_dt_call_warns_and_names_the_source() {
    let source = r#"
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := CURRENT_DT();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        found[0].message,
        "CURRENT_DT reads the host clock, which replay does not control"
    );
    let expected = source.find("CURRENT_DT()").expect("call") as u32;
    assert_eq!(u32::from(found[0].range.start()), expected);
}

/// Specification 28 section 5: the call site of a function whose body reads
/// `CURRENT_DT` warns too. Wrong behaviour this pins: only the inner call
/// warning, with the outer call site silent.
#[test]
fn a_call_site_of_a_direct_clock_reader_warns() {
    let source = r#"
FUNCTION GetNow : DT
GetNow := CURRENT_DT();
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := GetNow();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().any(|diagnostic| diagnostic
            .message
            .contains("CURRENT_DT reads the host clock")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|diagnostic| diagnostic.message.contains("'GetNow'")
                && diagnostic.message.contains("CURRENT_DT")),
        "{found:?}"
    );
}

/// Specification 28 section 5: "directly or through the POUs it calls". Wrong
/// behaviour this pins: a two-step chain losing the outer call site.
#[test]
fn a_transitive_clock_reader_warns_at_the_outer_call_site() {
    let source = r#"
FUNCTION Inner : DT
Inner := CURRENT_DT();
END_FUNCTION
FUNCTION Outer : DT
Outer := Inner();
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := Outer();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        found
            .iter()
            .any(|diagnostic| diagnostic.message.contains("'Outer'")
                && diagnostic.message.contains("CURRENT_DT")),
        "{found:?}"
    );
}

/// Specification 28 section 5 and specification 07: `TIME` returns the
/// runtime's logical elapsed time, which replay controls; `T_PLC_MS` builds on
/// it. Neither warns, and neither does an unknown vendor clock function whose
/// body is not in the project.
#[test]
fn runtime_and_unknown_clock_calls_do_not_warn() {
    let source = r#"
PROGRAM Main
VAR
    Uptime : TIME;
    Tick : DINT;
    Stamp : DT;
END_VAR
Uptime := TIME();
Tick := T_PLC_MS();
Stamp := RTC_MS();
END_PROGRAM
"#;
    assert!(time_warnings(source).is_empty());
}

/// Specification 28 section 5: a body-less library stub reads no clock, so its
/// callers do not warn (the vendor-stub corpus: 79 declaration warnings to
/// zero). Wrong behaviour this pins: a declaration-only stub falling back to a
/// warning.
#[test]
fn a_bodyless_library_stub_call_does_not_warn() {
    let source = r#"
FUNCTION FuGetDateAndTime : DT
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := FuGetDateAndTime();
END_PROGRAM
"#;
    assert!(time_warnings(source).is_empty());
}

/// Specification 28 section 5: the call graph crosses files. The library file
/// keeps its own clock-read warning (the #180 library-surface decision), and
/// the consumer's call site warns in the consumer file.
#[test]
fn a_clock_reader_in_another_file_warns_at_the_consumer_call_site() {
    let sources = [
        r#"
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := CURRENT_DT();
END_FUNCTION
"#,
        r#"
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := FuGetDateAndTime();
END_PROGRAM
"#,
    ];
    let owner = time_warnings_in(&sources, 0);
    let consumer = time_warnings_in(&sources, 1);

    assert_eq!(owner.len(), 1, "{owner:?}");
    assert_eq!(consumer.len(), 1, "{consumer:?}");
    assert!(
        consumer[0].message.contains("'FuGetDateAndTime'"),
        "{consumer:?}"
    );
    assert!(consumer[0].message.contains("CURRENT_DT"), "{consumer:?}");
}

/// Specification 28 section 5: methods are tracked like functions. Wrong
/// behaviour this pins: a method body reaching `CURRENT_DT` while its call
/// sites stay silent.
#[test]
fn a_method_call_on_a_clock_reading_block_warns_at_the_call() {
    let source = r#"
FUNCTION_BLOCK WallClock
METHOD PUBLIC Read : DT
Read := CURRENT_DT();
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Clock : WallClock;
    Stamp : DT;
END_VAR
Stamp := Clock.Read();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .any(|diagnostic| diagnostic.message.contains("'Read'")
                && diagnostic.message.contains("CURRENT_DT")),
        "{found:?}"
    );
}

/// Specification 28 section 5: function blocks are tracked like functions.
/// Wrong behaviour this pins: an FB body reading the clock while its
/// invocation stays silent.
#[test]
fn an_fb_call_of_a_clock_reading_block_warns() {
    let source = r#"
FUNCTION_BLOCK Ticker
VAR
    Seen : DT;
END_VAR
Seen := CURRENT_DT();
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Ticker1 : Ticker;
END_VAR
Ticker1();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .any(|diagnostic| diagnostic.message.contains("'Ticker'")
                && diagnostic.message.contains("CURRENT_DT")),
        "{found:?}"
    );
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
fn direct_input_and_output_declarations_in_reusable_code_warn_independently() {
    let found = warnings(
        r#"
FUNCTION_BLOCK Station
VAR
    InputBit AT %IX0.0 : BOOL;
    InputWord AT %IW2 : WORD;
    OutputBit AT %QX0.0 : BOOL;
    OutputWord AT %QW2 : WORD;
END_VAR
END_FUNCTION_BLOCK
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
fn a_programs_io_list_does_not_warn() {
    let found = warnings(
        r#"
PROGRAM Main
VAR
    LowWater AT %IX0.0 : BOOL;
    Temperature AT %IL8 : LREAL;
    PumpRun AT %QX0.0 : BOOL;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::NondeterministicIo,
    );

    assert!(found.is_empty(), "{found:?}");
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

/// Wrong behaviour this pins: the old rule reporting both codes on one
/// declaration. Specification 28 section 5: the DT storage no longer warns;
/// only the I/O address does.
#[test]
fn a_time_date_declaration_with_io_address_reports_only_the_io_hazard() {
    let source = r#"
FUNCTION_BLOCK Clock
VAR
    ExternalClock AT %QD0 : DT;
END_VAR
END_FUNCTION_BLOCK
"#;
    let time = time_warnings(source);
    let io = warnings(source, DiagnosticCode::NondeterministicIo);

    assert!(time.is_empty(), "{time:?}");
    assert_eq!(io.len(), 1);
}

/// Wrong behaviour this pins: a clock read reaching the outer call site only
/// through an FB instance inside another FB is lost there. Specification 28
/// section 5: the call site of a function block whose body reaches the clock,
/// directly or through the POUs it calls, warns. An instance's call maps to
/// the block the variable declares.
#[test]
fn a_clock_read_through_fb_instances_warns_at_every_level_up() {
    let source = r#"
FUNCTION_BLOCK Ticker
VAR
    Seen : DT;
END_VAR
Seen := CURRENT_DT();
END_FUNCTION_BLOCK
FUNCTION_BLOCK Wrapper
VAR
    Inner : Ticker;
END_VAR
Inner();
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Outer : Wrapper;
END_VAR
Outer();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        found
            .iter()
            .any(|diagnostic| diagnostic.message.contains("'Ticker'")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|diagnostic| diagnostic.message.contains("'Wrapper'")),
        "{found:?}"
    );
}

/// Wrong behaviour this pins: a bare call taking a same-named method's clock.
/// `Read()` inside `Caller` binds to the function `Read`, which reads no
/// clock; a method is never reachable by bare name alone (specification 28
/// section 5), so `Caller` stays silent.
#[test]
fn a_bare_call_never_takes_a_same_named_methods_clock() {
    let source = r#"
FUNCTION_BLOCK WallClock
METHOD PUBLIC Read : DT
Read := CURRENT_DT();
END_METHOD
END_FUNCTION_BLOCK
FUNCTION Read : INT
Read := 1;
END_FUNCTION
FUNCTION Caller : INT
Caller := Read();
END_FUNCTION
PROGRAM Main
VAR
    Ticks : INT;
END_VAR
Ticks := Caller();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].message.contains("CURRENT_DT reads the host clock"),
        "{found:?}"
    );
}

/// Wrong behaviour this pins: the graph treating a call of a user-declared
/// `CURRENT_DT` function as the host clock. The call-site check honours the
/// shadowing; the graph follows the same rule (specification 28 section 5:
/// the source is the product function, not a same-named declaration).
#[test]
fn a_user_declared_current_dt_shadows_the_host_clock() {
    let source = r#"
FUNCTION CURRENT_DT : DT
CURRENT_DT := DT#1970-01-01-00:00:00;
END_FUNCTION
FUNCTION GetNow : DT
GetNow := CURRENT_DT();
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := GetNow();
END_PROGRAM
"#;
    assert!(time_warnings(source).is_empty());
}

/// Wrong behaviour this pins: a namespace-qualified call of a clock-reading
/// library function staying silent at its call site, or failing to carry the
/// clock through a wrapper that calls it. Specification 28 section 5: the
/// call site of a function whose body reaches the clock warns, and the vendor
/// corpus calls `WagoAppTime.FuGetDateAndTime()` exactly this way.
#[test]
fn a_namespace_qualified_call_of_a_clock_reader_warns() {
    let source = r#"
NAMESPACE WagoAppTime
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := CURRENT_DT();
END_FUNCTION
END_NAMESPACE
FUNCTION GetNow : DT
GetNow := WagoAppTime.FuGetDateAndTime();
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := WagoAppTime.FuGetDateAndTime();
Stamp := GetNow();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 4, "{found:?}");
    assert!(
        found.iter().any(
            |diagnostic| diagnostic.message.contains("'FuGetDateAndTime'")
                && diagnostic.message.contains("CURRENT_DT")
        ),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|diagnostic| diagnostic.message.contains("'GetNow'")),
        "{found:?}"
    );
}

/// Specification 07 "Declared Clock Sources" and specification 28 section 5:
/// a body-less vendor stub carrying `{attribute 'reads_host_clock'}` declares
/// that it samples the host clock, so its call sites warn and name it. Wrong
/// behaviour this pins: the declaration staying unread (the vendor corpus
/// measures zero W010s behind the stub boundary), or the stub's own file
/// warning — the declaration speaks for the missing body, and the warnings
/// belong at the call sites, as with a linked library body under #180.
#[test]
fn an_attributed_bodyless_stub_warns_at_its_call_site_and_names_the_stub() {
    let sources = [
        r#"
{attribute 'reads_host_clock'}
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := FuGetDateAndTime;
END_FUNCTION
"#,
        r#"
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := FuGetDateAndTime();
END_PROGRAM
"#,
    ];
    let stub_file = time_warnings_in(&sources, 0);
    let app_file = time_warnings_in(&sources, 1);

    assert!(
        stub_file.is_empty(),
        "the declared stub's own file must not warn: {stub_file:?}"
    );
    assert_eq!(app_file.len(), 1, "{app_file:?}");
    assert!(
        app_file[0].message.contains("'FuGetDateAndTime'")
            && app_file[0].message.contains("reads_host_clock"),
        "{app_file:?}"
    );
}

/// Specification 07: the declaration is read before the POU's declaration
/// keyword, so the attribute works inside a `NAMESPACE`, and the call site
/// resolves the qualified call to the declared stub. Wrong behaviour this
/// pins: the pragma going unread because it sits in a namespace or is
/// trivia the POU node does not own.
#[test]
fn a_declared_stub_inside_a_namespace_warns_at_the_qualified_call() {
    let source = r#"
NAMESPACE WagoAppTime
{attribute 'reads_host_clock'}
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := FuGetDateAndTime;
END_FUNCTION
END_NAMESPACE
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := WagoAppTime.FuGetDateAndTime();
END_PROGRAM
"#;
    let found = time_warnings(source);

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].message.contains("'FuGetDateAndTime'")
            && found[0]
                .message
                .contains("declares reads_host_clock and reads the host clock"),
        "{found:?}"
    );
}

/// Specification 28 section 5: the declared source crosses function blocks,
/// and the message names the source the call reaches instead of a hardcoded
/// `CURRENT_DT`. Wrong behaviour this pins: the block's invocation staying
/// silent, or naming `CURRENT_DT` which the stub never calls (issue #264).
#[test]
fn a_declared_stub_warns_through_an_fb_instance_call_and_names_the_source() {
    let sources = [
        r#"
{attribute 'reads_host_clock'}
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := FuGetDateAndTime;
END_FUNCTION
"#,
        r#"
FUNCTION_BLOCK MqttSub
VAR
    T : DT;
END_VAR
T := FuGetDateAndTime();
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Handle : MqttSub;
END_VAR
Handle();
END_PROGRAM
"#,
    ];
    let app_file = time_warnings_in(&sources, 1);

    assert_eq!(app_file.len(), 2, "{app_file:?}");
    assert!(
        app_file.iter().any(|diagnostic| diagnostic
            .message
            .contains("'FuGetDateAndTime' declares reads_host_clock")),
        "{app_file:?}"
    );
    assert!(
        app_file.iter().any(|diagnostic| diagnostic
            .message
            .contains("'MqttSub' reads the host clock through 'FuGetDateAndTime', which declares reads_host_clock")),
        "{app_file:?}"
    );
}

/// Specification 07: a POU whose body reads `CURRENT_DT` and that also
/// carries the attribute is one source, not two. Wrong behaviour this pins:
/// the attribute seeding a second warning or hiding the concrete read behind
/// the declaration.
#[test]
fn a_real_body_reading_current_dt_with_the_attribute_warns_once() {
    let sources = [
        r#"
{attribute 'reads_host_clock'}
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := CURRENT_DT();
END_FUNCTION
"#,
        r#"
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := FuGetDateAndTime();
END_PROGRAM
"#,
    ];
    let stub_file = time_warnings_in(&sources, 0);
    let app_file = time_warnings_in(&sources, 1);

    assert_eq!(stub_file.len(), 1, "{stub_file:?}");
    assert!(
        stub_file[0]
            .message
            .contains("CURRENT_DT reads the host clock"),
        "{stub_file:?}"
    );
    assert_eq!(app_file.len(), 1, "{app_file:?}");
    assert!(
        app_file[0]
            .message
            .contains("reads the host clock through CURRENT_DT"),
        "{app_file:?}"
    );
}

/// Specification 28 section 5: the attribute follows the resolved
/// declaration, not the spelling. Wrong behaviour this pins: a bare call of
/// a user-declared function that shadows the stub's name warning because the
/// stub carries the attribute.
#[test]
fn a_user_declared_function_shadowing_the_stub_keeps_the_shadowing_rule() {
    let sources = [
        r#"
NAMESPACE WagoAppTime
{attribute 'reads_host_clock'}
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := FuGetDateAndTime;
END_FUNCTION
END_NAMESPACE
"#,
        r#"
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := DT#1970-01-01-00:00:00;
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := FuGetDateAndTime();
Stamp := WagoAppTime.FuGetDateAndTime();
END_PROGRAM
"#,
    ];
    let app_file = time_warnings_in(&sources, 1);

    assert_eq!(app_file.len(), 1, "{app_file:?}");
    assert!(
        app_file[0].message.contains("'FuGetDateAndTime'")
            && app_file[0].message.contains("declares reads_host_clock"),
        "{app_file:?}"
    );
}

/// Specification 07: the declaration is read from the pragma trivia before
/// the POU's declaration keyword, never from the body. Wrong behaviour this
/// pins: a pragma inside the body being taken for a declaration.
#[test]
fn an_attribute_inside_the_body_does_not_declare_a_source() {
    let source = r#"
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := FuGetDateAndTime;
{attribute 'reads_host_clock'}
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := FuGetDateAndTime();
END_PROGRAM
"#;
    assert!(time_warnings(source).is_empty());
}

/// Specification 28 section 5: a call that reaches more than one source
/// names the one whose direct reader comes first in the project's file order
/// and document order. Wrong behaviour this pins: the message naming a
/// source picked by hash-map iteration order.
#[test]
fn a_call_reaching_two_declared_sources_names_the_first_in_document_order() {
    let sources = [
        r#"
{attribute 'reads_host_clock'}
FUNCTION FuGetDateAndTime : DT
FuGetDateAndTime := FuGetDateAndTime;
END_FUNCTION
{attribute 'reads_host_clock'}
FUNCTION FuGetLocalDateAndTime : DT
FuGetLocalDateAndTime := FuGetLocalDateAndTime;
END_FUNCTION
"#,
        r#"
FUNCTION Wrapper : DT
VAR
    A : DT;
    B : DT;
END_VAR
A := FuGetDateAndTime();
B := FuGetLocalDateAndTime();
Wrapper := A;
END_FUNCTION
PROGRAM Main
VAR
    Stamp : DT;
END_VAR
Stamp := Wrapper();
END_PROGRAM
"#,
    ];
    let app_file = time_warnings_in(&sources, 1);

    // Every call site of a reader warns: the two calls inside Wrapper and
    // Wrapper's own call site.
    assert_eq!(app_file.len(), 3, "{app_file:?}");
    let wrapper_call = app_file
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("'Wrapper' reads the host clock through")
        })
        .expect("{app_file:?}");
    assert!(
        wrapper_call
            .message
            .contains("through 'FuGetDateAndTime', which declares reads_host_clock")
            && !wrapper_call.message.contains("FuGetLocalDateAndTime"),
        "{wrapper_call:?}"
    );
    assert!(
        app_file.iter().any(|diagnostic| diagnostic
            .message
            .contains("'FuGetLocalDateAndTime' declares reads_host_clock")),
        "{app_file:?}"
    );
}
