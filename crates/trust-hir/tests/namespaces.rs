mod common;

use common::*;
use smol_str::SmolStr;

#[test]
fn iec_table64() {
    check_no_errors(
        r#"
NAMESPACE Lib
FUNCTION Inc : INT
VAR_INPUT
    x : INT;
END_VAR
Inc := x + INT#1;
END_FUNCTION
END_NAMESPACE

PROGRAM Main
VAR
    y : INT;
END_VAR
y := Lib.Inc(INT#1);
END_PROGRAM
"#,
    );
}

#[test]
fn iec_table66() {
    check_no_errors(
        r#"
NAMESPACE Lib
FUNCTION Inc : INT
VAR_INPUT
    x : INT;
END_VAR
Inc := x + INT#1;
END_FUNCTION
END_NAMESPACE

USING Lib;
PROGRAM Main
VAR
    y : INT;
END_VAR
y := Inc(INT#1);
END_PROGRAM
"#,
    );
}

#[test]
fn using_imports_namespaced_enum_type_for_typed_literals_across_files() {
    check_no_errors_multi(&[
        r#"
USING Control;
PROGRAM Main
VAR
    Mode : Control.PIDMode;
END_VAR
Mode := PIDMode#Manual;
END_PROGRAM
"#,
        r#"
NAMESPACE Control
TYPE PIDMode : (Auto, Manual, Track);
END_TYPE
END_NAMESPACE
"#,
    ]);
}

#[test]
fn typed_enum_literal_keeps_type_identity_when_a_variable_has_the_same_name() {
    check_no_errors(
        r#"
TYPE Color : (Red, Green, Blue);
END_TYPE

PROGRAM Main
VAR
    Color : Color;
END_VAR
Color := Color#Green;
END_PROGRAM
"#,
    );
}

#[test]
fn namespace_global_supports_qualified_reads_and_writes() {
    check_no_errors(
        r#"
NAMESPACE GVL
VAR_GLOBAL
    shared : INT := 1;
END_VAR
END_NAMESPACE

PROGRAM Main
VAR
    observed : INT;
END_VAR
observed := GVL.shared;
GVL.shared := observed + 1;
END_PROGRAM
"#,
    );
}

#[test]
fn namespaced_type_reference_prefers_scoped_type_over_global_bare_name() {
    check_no_errors(
        r#"
TYPE LocalType : STRUCT
    global_only : BOOL;
END_STRUCT
END_TYPE

NAMESPACE CellA
TYPE LocalType : STRUCT
    scoped_value : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    item : LocalType;
    result : INT;
END_VAR
result := item.scoped_value;
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn namespaced_extends_member_resolution_prefers_scoped_base_over_global() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Base
VAR
    global_only : DINT;
END_VAR
END_FUNCTION_BLOCK

NAMESPACE CellA
FUNCTION_BLOCK Base
VAR PUBLIC
    scoped_value : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    item : Derived;
    result : INT;
END_VAR
result := item.scoped_value;
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn same_named_pou_constants_in_different_namespaces_do_not_collide() {
    check_no_errors(
        r#"
NAMESPACE CellA
PROGRAM Main
VAR CONSTANT
    Limit : INT := 2;
END_VAR
VAR
    values : ARRAY[0..Limit] OF INT;
END_VAR
END_PROGRAM
END_NAMESPACE

NAMESPACE CellB
PROGRAM Main
VAR CONSTANT
    Limit : INT := 4;
END_VAR
VAR
    values : ARRAY[0..Limit] OF INT;
END_VAR
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn same_named_pou_constants_in_different_namespaces_keep_distinct_values() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
NAMESPACE CellA
PROGRAM Main
VAR CONSTANT
    Limit : INT := 2;
END_VAR
VAR
    values : ARRAY[0..Limit] OF INT;
END_VAR
END_PROGRAM
END_NAMESPACE

NAMESPACE CellB
PROGRAM Main
VAR CONSTANT
    Limit : INT := 4;
END_VAR
VAR
    values : ARRAY[0..Limit] OF INT;
END_VAR
END_PROGRAM
END_NAMESPACE
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let array_bounds = |namespace: &str| {
        let owner = symbols
            .resolve_qualified(&[SmolStr::new(namespace), SmolStr::new("Main")])
            .unwrap_or_else(|| panic!("{namespace}.Main should resolve"));
        let values = symbols
            .iter()
            .find(|symbol| symbol.name == "values" && symbol.parent == Some(owner))
            .unwrap_or_else(|| panic!("{namespace}.Main.values should be collected"));
        let type_id = symbols.resolve_alias_type(values.type_id);
        let Type::Array { dimensions, .. } = symbols.type_by_id(type_id).expect("array type")
        else {
            panic!("{namespace}.Main.values should be an array");
        };
        dimensions.clone()
    };

    assert_eq!(array_bounds("CellA"), vec![(0, 2)]);
    assert_eq!(array_bounds("CellB"), vec![(0, 4)]);
}

#[test]
fn using_ambiguous_name_reports_cannot_resolve_without_choosing_candidate() {
    let errors = check_errors(
        r#"
NAMESPACE A
FUNCTION Pick : INT
    Pick := INT#1;
END_FUNCTION
END_NAMESPACE

NAMESPACE B
FUNCTION Pick : INT
    Pick := INT#2;
END_FUNCTION
END_NAMESPACE

USING A;
USING B;

PROGRAM Main
VAR
    result : INT;
END_VAR
result := Pick();
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::CannotResolve),
        "expected CannotResolve for ambiguous USING candidate, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::UndefinedFunction),
        "ambiguous USING must not degrade into wrong-reason UndefinedFunction, got {errors:?}"
    );
}

#[test]
fn using_ambiguous_value_reports_cannot_resolve_without_undefined_variable() {
    let errors = check_errors(
        r#"
NAMESPACE A
VAR_GLOBAL
    Shared : INT;
END_VAR
END_NAMESPACE

NAMESPACE B
VAR_GLOBAL
    Shared : INT;
END_VAR
END_NAMESPACE

USING A;
USING B;

PROGRAM Main
VAR
    result : INT;
END_VAR
result := Shared;
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::CannotResolve),
        "expected CannotResolve for ambiguous USING value candidate, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::UndefinedVariable),
        "ambiguous USING value must not degrade into wrong-reason UndefinedVariable, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::TypeMismatch),
        "ambiguous USING value must not produce a follow-on TypeMismatch, got {errors:?}"
    );
}

#[test]
fn using_ambiguous_assignment_target_reports_one_primary() {
    let errors = check_errors(
        r#"
NAMESPACE A
VAR_GLOBAL
    Shared : INT;
END_VAR
END_NAMESPACE

NAMESPACE B
VAR_GLOBAL
    Shared : INT;
END_VAR
END_NAMESPACE

USING A;
USING B;

PROGRAM Main
Shared := 1;
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|code| **code == DiagnosticCode::CannotResolve)
            .count(),
        1,
        "ambiguous assignment target must emit one primary CannotResolve, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::UndefinedVariable),
        "ambiguous assignment target must not degrade into UndefinedVariable, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::TypeMismatch),
        "ambiguous assignment target must not produce a follow-on TypeMismatch, got {errors:?}"
    );
}

#[test]
fn cross_file_using_ambiguous_name_reports_cannot_resolve_without_choosing_import() {
    let mut db = Database::new();
    let file_a = FileId(0);
    let file_b = FileId(1);
    let file_main = FileId(2);
    db.set_source_text(
        file_a,
        r#"
NAMESPACE A
FUNCTION Pick : INT
    Pick := INT#1;
END_FUNCTION
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        file_b,
        r#"
NAMESPACE B
FUNCTION Pick : INT
    Pick := INT#2;
END_FUNCTION
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        file_main,
        r#"
USING A;
USING B;

PROGRAM Main
VAR
    result : INT;
END_VAR
result := Pick();
END_PROGRAM
"#
        .to_string(),
    );

    let errors = db
        .diagnostics(file_main)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert!(
        errors.contains(&DiagnosticCode::CannotResolve),
        "expected CannotResolve for ambiguous cross-file USING candidate, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::UndefinedFunction),
        "ambiguous cross-file USING must not degrade into wrong-reason UndefinedFunction, got {errors:?}"
    );
}

#[test]
fn namespace_qualified_type_reference_matches_type_check_resolution() {
    check_no_errors(
        r#"
NAMESPACE Lib
TYPE Payload : STRUCT
    value : DINT;
END_STRUCT
END_TYPE
END_NAMESPACE

PROGRAM Main
VAR
    payload : Lib.Payload;
    value : DINT;
END_VAR
payload.value := DINT#7;
value := payload.value;
END_PROGRAM
"#,
    );
}

// IEC 61131-3 Ed.3 §6.4.4.2 and §6.9: an enumeration value needs its type's prefix only where two
// enumerated types the use can see define it; a type of a namespace the use neither is in nor
// imports does not make it ambiguous.
#[test]
fn an_enum_value_of_another_namespace_does_not_make_a_value_ambiguous() {
    check_no_errors(
        r#"
NAMESPACE Control
TYPE PIDMode : (Auto, Manual, Track); END_TYPE
FUNCTION Pick : BOOL
VAR_INPUT
    Mode : PIDMode;
END_VAR
Pick := Mode = Manual;
END_FUNCTION
END_NAMESPACE

NAMESPACE Equipment
TYPE EquipmentMode : (Auto, Manual, OutOfService); END_TYPE
END_NAMESPACE

PROGRAM Main
VAR
    Mode : Control.PIDMode;
    On : BOOL;
END_VAR
On := Control.Pick(Mode);
END_PROGRAM
"#,
    );
}

#[test]
fn an_enum_value_two_types_of_its_namespace_define_still_needs_its_type() {
    check_has_error(
        r#"
NAMESPACE Equipment
TYPE EquipmentMode : (Auto, Manual, OutOfService); END_TYPE
TYPE LoopMode : (Manual, Auto, Cascade); END_TYPE
FUNCTION Pick : BOOL
VAR_INPUT
    Mode : LoopMode;
END_VAR
Pick := Mode = Manual;
END_FUNCTION
END_NAMESPACE
"#,
        DiagnosticCode::CannotResolve,
    );
}

// IEC 61131-3 Ed.3 §6.5.1 (initial values) with §6.6.5.5.2 rule 2 and §6.6.7.2.7 (a derived
// function block inherits all variables of its base): an instance's initialiser may set the
// members its function block declares and those it inherits, also when
// the function block is declared in another file of the project, inside a namespace, and named
// either qualified or through USING.
const INHERITING_LIBRARY: &str = r#"
NAMESPACE Plant
FUNCTION_BLOCK FB_Base
VAR_INPUT
    Limit : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR_INPUT
    Own : INT;
END_VAR
END_FUNCTION_BLOCK
END_NAMESPACE
"#;

#[test]
fn an_initialiser_sets_inherited_members_of_a_block_from_another_file() {
    check_no_errors_multi(&[
        INHERITING_LIBRARY,
        r#"
USING Plant;

PROGRAM Main
VAR
    Qualified : Plant.FB_Derived := (Limit := 1, Own := 2);
    Imported : FB_Derived := (Limit := 3, Own := 4);
END_VAR
Qualified();
Imported();
END_PROGRAM
"#,
    ]);
}

#[test]
fn an_initialiser_member_no_block_of_the_chain_declares_is_refused() {
    let mut db = Database::new();
    db.set_source_text(FileId(0), INHERITING_LIBRARY.to_string());
    db.set_source_text(
        FileId(1),
        r#"
USING Plant;

PROGRAM Main
VAR
    Imported : FB_Derived := (Nope := 1);
END_VAR
Imported();
END_PROGRAM
"#
        .to_string(),
    );
    let errors: Vec<DiagnosticCode> = db
        .diagnostics(FileId(1))
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| d.code)
        .collect();
    assert_eq!(errors, vec![DiagnosticCode::UndefinedField]);
}
