//! An input of a method that implements a method prototype of an interface is not reported as unused
//! (W002, specification 28 §1): the interface fixes the method's signature, so the implementing method
//! declares the input even where its body does not read it. A block implements the interfaces that
//! its IMPLEMENTS names and those that the blocks it extends implement (IEC 61131-3 Ed.3 §6.6.6.4),
//! and an interface's prototypes are its own and those it inherits through EXTENDS, at any depth
//! (§6.6.6.6.1, rules 1 and 4). Contributor issue #482: a block implementing
//! `itfMqttPayloadBuilder EXTENDS itfMqttBuild, itfMqttResult` got W002 for the inputs of the
//! prototypes of `itfMqttBuild`, which the CODESYS 3.5.17 build of the same project does not warn
//! about.

use crate::common::*;

/// The W002 messages of `source` under `dialect`, sorted, once the program is shown to have no error.
fn unused_parameters(dialect: Dialect, source: &str) -> Vec<String> {
    assert_eq!(
        check_errors_in(dialect, source),
        vec![],
        "the program is valid"
    );
    let mut db = Database::new();
    db.set_dialect(dialect);
    db.set_source_text(FileId(0), source.to_string());
    let mut messages: Vec<String> = db
        .diagnostics(FileId(0))
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedParameter)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    messages.sort();
    messages
}

/// Issue #482 under the CODESYS profile it came from: `fbMqttPayloadLight` implements the composed
/// interface, whose prototypes `mBuildState` and `mBuildDiscovery` come from its base `itfMqttBuild`,
/// and their bodies do not read `uiIdx` and `xEvent`. Only `iUnread`, an input of `mTrace`, which no
/// interface declares, warns: 1 warning. Catches the lookup that searched only the prototypes the
/// named interface declares itself (3 warnings: `iUnread`, `uiIdx` and `xEvent`), and an exemption
/// widened to every method of an implementing block (none).
#[test]
fn inputs_of_methods_implementing_prototypes_an_interface_inherits_do_not_warn() {
    let source = r#"
TYPE stEntity :
STRUCT
    uiId : UINT;
END_STRUCT
END_TYPE

TYPE stMqttMessage :
STRUCT
    sTopic : STRING;
END_STRUCT
END_TYPE

INTERFACE itfMqttBuild
METHOD mBuildState : BOOL
    VAR_INPUT uiIdx : UINT; END_VAR
    VAR_INPUT stEntityIn : stEntity; END_VAR
END_METHOD
METHOD mBuildDiscovery : BOOL
    VAR_INPUT xEvent : BOOL; END_VAR
END_METHOD
END_INTERFACE

INTERFACE itfMqttResult
METHOD mGetResult
    VAR_OUTPUT stMsg : stMqttMessage; END_VAR
    VAR_OUTPUT xReady : BOOL; END_VAR
END_METHOD
END_INTERFACE

INTERFACE itfMqttPayloadBuilder EXTENDS itfMqttBuild, itfMqttResult
END_INTERFACE

FUNCTION_BLOCK fbMqttPayloadBase
VAR
    stLast : stMqttMessage;
    xDone : BOOL;
END_VAR
METHOD mGetResult
    VAR_OUTPUT stMsg : stMqttMessage; END_VAR
    VAR_OUTPUT xReady : BOOL; END_VAR
    stMsg := stLast;
    xReady := xDone;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK fbMqttPayloadLight EXTENDS fbMqttPayloadBase IMPLEMENTS itfMqttPayloadBuilder
VAR
    uiState : UINT;
END_VAR
METHOD mBuildState : BOOL
    VAR_INPUT uiIdx : UINT; END_VAR
    VAR_INPUT stEntityIn : stEntity; END_VAR
    uiState := stEntityIn.uiId;
    mBuildState := TRUE;
END_METHOD
METHOD mBuildDiscovery : BOOL
    VAR_INPUT xEvent : BOOL; END_VAR
    mBuildDiscovery := TRUE;
END_METHOD
METHOD mTrace : BOOL
    VAR_INPUT iUnread : INT; END_VAR
    mTrace := TRUE;
END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_eq!(
        unused_parameters(Dialect::Codesys, source),
        vec!["unused parameter 'iUnread'"]
    );
}

/// `IConveyor` extends `IDrive`, which extends `IStart`, so `Start` is a prototype of `IConveyor` two
/// bases away (rule 4: a base passes on the prototypes it inherited), and `Stop` is its own.
/// `Conveyor` reads neither `Speed` nor `Ramp`: no warning. Catches a lookup that reads only the
/// named interface's own prototypes, and one that follows only its direct bases: either reports
/// `Speed`.
#[test]
fn a_prototype_inherited_through_two_interface_bases_exempts_its_implementation() {
    let source = r#"
INTERFACE IStart
METHOD Start : BOOL
VAR_INPUT
    Speed : REAL;
END_VAR
END_METHOD
END_INTERFACE

INTERFACE IDrive EXTENDS IStart
END_INTERFACE

INTERFACE IConveyor EXTENDS IDrive
METHOD Stop : BOOL
VAR_INPUT
    Ramp : TIME;
END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Conveyor IMPLEMENTS IConveyor
VAR
    Running : BOOL;
END_VAR
METHOD PUBLIC Start : BOOL
VAR_INPUT
    Speed : REAL;
END_VAR
    Running := TRUE;
    Start := Running;
END_METHOD
METHOD PUBLIC Stop : BOOL
VAR_INPUT
    Ramp : TIME;
END_VAR
    Running := FALSE;
    Stop := TRUE;
END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_eq!(
        unused_parameters(Dialect::Iec, source),
        Vec::<String>::new()
    );
}

/// `QuickValve` extends `Valve`, which implements `IValve`, so `QuickValve` implements `IValve`
/// (§6.6.6.4) and its `Open` is what a call of `IValve.Open` on a `QuickValve` runs: its unread `Ramp`
/// does not warn. `Purge`, a method that `QuickValve` adds, keeps the warning for `Seconds`: 1
/// warning. Catches a lookup that reads only the IMPLEMENTS of the method's own block (2 warnings:
/// `Ramp` and `Seconds`), and an exemption widened to every method of a derived block (none).
#[test]
fn an_override_in_a_block_derived_from_an_implementing_block_is_exempt_and_a_new_method_is_not() {
    let source = r#"
INTERFACE IValve
METHOD Open : BOOL
VAR_INPUT
    Ramp : TIME;
END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Valve IMPLEMENTS IValve
VAR
    Position : REAL;
    RampTime : TIME;
END_VAR
METHOD PUBLIC Open : BOOL
VAR_INPUT
    Ramp : TIME;
END_VAR
    RampTime := Ramp;
    Position := 100.0;
    Open := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK QuickValve EXTENDS Valve
METHOD PUBLIC OVERRIDE Open : BOOL
VAR_INPUT
    Ramp : TIME;
END_VAR
    Position := 100.0;
    Open := TRUE;
END_METHOD
METHOD PUBLIC Purge : BOOL
VAR_INPUT
    Seconds : TIME;
END_VAR
    Purge := TRUE;
END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_eq!(
        unused_parameters(Dialect::Iec, source),
        vec!["unused parameter 'Seconds'"]
    );
}
