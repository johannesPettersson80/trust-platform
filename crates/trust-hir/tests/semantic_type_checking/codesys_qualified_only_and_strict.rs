//! CODESYS's `{attribute 'qualified_only'}` and `{attribute 'strict'}` (specification 02 §3.1, specification 04
//! "Rules"; the vendor's attribute help and its strict programming rules for enumerations).

use crate::common::*;

/// The errors of `source` under `dialect`, each with its message.
fn errors(dialect: Dialect, source: &str) -> Vec<(DiagnosticCode, String)> {
    let mut db = Database::new();
    db.set_dialect(dialect);
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| (diagnostic.code, diagnostic.message.clone()))
        .collect()
}

const MODE: &str = "TYPE E_Mode : (MANUAL, AUTO);\nEND_TYPE\n";

/// Specification 02 §3.1: a value of a `qualified_only` enumeration is named with its type. The program names MANUAL bare
/// in an initializer, an assignment and a `CASE` label, and AUTO qualified in both forms: the three bare names are errors
/// (E105) that name `E_Mode.MANUAL`, and the qualified ones are taken. Catches a bare value that is taken although the
/// type is `qualified_only`, a qualified value that is refused, and a message that does not say how to write the name.
#[test]
fn a_qualified_only_enumeration_takes_its_values_only_qualified() {
    let program = "PROGRAM P\nVAR\n    m : E_Mode := MANUAL;\n    n : INT;\nEND_VAR\nm := MANUAL;\nm := E_Mode.AUTO;\n\
                   m := E_Mode#AUTO;\nCASE m OF\n    MANUAL: n := 1;\n    E_Mode.AUTO: n := 2;\nEND_CASE\nEND_PROGRAM\n";
    let message =
        "'MANUAL' is a value of E_Mode, which is {attribute 'qualified_only'}: write E_Mode.MANUAL";
    assert_eq!(
        errors(
            Dialect::Codesys,
            &format!("{{attribute 'qualified_only'}}\n{MODE}{program}")
        ),
        vec![(DiagnosticCode::CannotResolve, message.to_string()); 3]
    );
    assert_eq!(
        errors(Dialect::Codesys, &format!("{MODE}{program}")),
        Vec::new()
    );
}

const GVL: &str = "NAMESPACE GVL_IO\nVAR_GLOBAL\n    lrLevel : LREAL;\nEND_VAR\n\nFUNCTION Level : LREAL\n\
                   Level := lrLevel;\nEND_FUNCTION\nEND_NAMESPACE\n\nPROGRAM P\nUSING GVL_IO;\nVAR\n    x : LREAL;\nEND_VAR\n\
                   x := lrLevel;\nx := GVL_IO.lrLevel;\nx := Level();\nEND_PROGRAM\n";

/// Specification 04, "Rules", 7: the globals of a namespace's `qualified_only` list are named with the namespace. The
/// program reaches `lrLevel` bare through its `USING` once, an error (E105) that names `GVL_IO.lrLevel`; the qualified
/// name is taken, and the namespace's own function names it bare. Catches a bare global that a `USING` makes visible
/// although its list is `qualified_only`, the namespace's own code refused, and the attribute taking effect on a list
/// that does not carry it.
#[test]
fn a_qualified_only_global_list_is_named_with_its_namespace_from_outside() {
    let qualified = GVL.replacen("VAR_GLOBAL", "{attribute 'qualified_only'}\nVAR_GLOBAL", 1);
    let message = "'lrLevel' is a global of the namespace GVL_IO, whose list is {attribute 'qualified_only'}: \
                   write GVL_IO.lrLevel";
    assert_eq!(
        errors(Dialect::Codesys, &qualified),
        vec![(DiagnosticCode::CannotResolve, message.to_string())]
    );
    assert_eq!(errors(Dialect::Codesys, GVL), Vec::new());
}

const LEVEL: &str = "TYPE E_Level : INT (LOW := 1, HIGH := 2);\nEND_TYPE\n";

/// Specification 02 §3.1: arithmetic with a value of a `strict` enumeration is an error (E202), also for a type with named
/// values, whose arithmetic §3.2 otherwise allows; reading the value as its integer base stays valid. `l + 1` is the one
/// error with the attribute, and nothing is refused without it. (A base-less enumeration takes no arithmetic with or
/// without the attribute.) Catches arithmetic taken although the type is `strict`, a read as INT refused, and arithmetic
/// refused for a type with named values that does not carry the attribute.
#[test]
fn a_strict_enumeration_refuses_arithmetic_and_still_reads_as_its_base() {
    let program = "PROGRAM P\nVAR\n    l : E_Level;\n    i : INT;\nEND_VAR\ni := l + 1;\ni := l;\nEND_PROGRAM\n";
    assert_eq!(
        errors(
            Dialect::Codesys,
            &format!("{{attribute 'strict'}}\n{LEVEL}{program}")
        ),
        vec![(
            DiagnosticCode::InvalidOperation,
            "arithmetic with a value of E_Level, which is {attribute 'strict'}".to_string()
        )]
    );
    assert_eq!(
        errors(Dialect::Codesys, &format!("{LEVEL}{program}")),
        Vec::new()
    );
}

/// Specification 02 §3.1: a `strict` enumeration with named values takes a constant that is one of its values and no
/// other. 2 is HIGH's value and is taken; 5 is none of its values, and the INT variable `i` is no constant, two errors
/// (E203). Without the attribute §3.2 takes all three. Catches a non-member constant or another type's variable taken
/// although the type is `strict`, a member's value refused, and the rule taking effect without the attribute.
#[test]
fn a_strict_enumeration_takes_only_its_own_values() {
    let program = "PROGRAM P\nVAR\n    l : E_Level;\n    i : INT;\nEND_VAR\nl := 2;\nl := 5;\nl := i;\nEND_PROGRAM\n";
    assert_eq!(
        errors(Dialect::Codesys, &format!("{{attribute 'strict'}}\n{LEVEL}{program}")),
        vec![
            (
                DiagnosticCode::IncompatibleAssignment,
                "5 is none of the values of E_Level, which is {attribute 'strict'}".to_string()
            ),
            (
                DiagnosticCode::IncompatibleAssignment,
                "only a value of E_Level or a constant that is one of its values can be assigned to it, as it is \
                 {attribute 'strict'}"
                    .to_string()
            ),
        ]
    );
    assert_eq!(
        errors(Dialect::Codesys, &format!("{LEVEL}{program}")),
        Vec::new()
    );
}
