//! Rust POUs in the semantic model (specification 48 RU-23, RU-31): their generated declarations are
//! checked as any other, a Structured Text call of one is checked against its declaration, the
//! symbol table marks it, and the checks that read a body pass over it, its body being its Rust
//! item.

mod common;
use common::*;

/// RU-22's function MAX_LIMITED, as RU-23 generates its declaration.
const MAX_LIMITED: &str = "{attribute 'rust'}
FUNCTION MAX_LIMITED : DINT
VAR_INPUT
    a : DINT;
    b : DINT;
    limit : DINT;
END_VAR
END_FUNCTION
";

/// RU-23's generated declaration of FILTER_EMA.
const FILTER_EMA: &str = "{attribute 'rust'}
FUNCTION_BLOCK FILTER_EMA
VAR_INPUT
    x : LREAL;
    alpha : LREAL := 0.1;
END_VAR
VAR_OUTPUT
    y : LREAL;
END_VAR
VAR
    primed : BOOL;
END_VAR
END_FUNCTION_BLOCK
";

#[test]
fn a_rust_functions_declaration_has_none_of_a_bodys_findings() {
    // Catches the missing result (E206) and the unused inputs (W002) that the same declaration has
    // as a Structured Text function: a Rust function's result and inputs are its Rust item's.
    assert_eq!(check_errors(MAX_LIMITED), Vec::<DiagnosticCode>::new());
    assert!(!check_warnings(MAX_LIMITED).contains(&DiagnosticCode::UnusedParameter));
    let structured_text = MAX_LIMITED.replace("{attribute 'rust'}\n", "");
    assert!(check_errors(&structured_text).contains(&DiagnosticCode::MissingReturn));
    assert!(check_warnings(&structured_text).contains(&DiagnosticCode::UnusedParameter));
}

#[test]
fn a_rust_programs_locals_are_not_unused() {
    // Catches W001 on a Rust program's variables, which its Rust item reads and writes (RU-35).
    let rust =
        "{attribute 'rust'}\nPROGRAM AXIS\nVAR\n    position : LREAL;\nEND_VAR\nEND_PROGRAM\n";
    assert_eq!(check_errors(rust), Vec::<DiagnosticCode>::new());
    assert!(!check_warnings(rust).contains(&DiagnosticCode::UnusedVariable));
    let structured_text = rust.replace("{attribute 'rust'}\n", "");
    assert!(check_warnings(&structured_text).contains(&DiagnosticCode::UnusedVariable));
}

#[test]
fn a_structured_text_call_of_a_rust_pou_is_checked_against_its_declaration() {
    // RU-23: the generated declaration is what Structured Text calls; a wrong input is an error as
    // for any POU.
    check_no_errors(&format!(
        "{MAX_LIMITED}{FILTER_EMA}PROGRAM Main\nVAR\n    m : DINT;\n    level : LREAL;\n    \
         filter : FILTER_EMA;\nEND_VAR\nm := MAX_LIMITED(a := 1, b := 2, limit := 3);\n\
         filter(x := 1.5);\nlevel := filter.y;\nEND_PROGRAM\n"
    ));
    for call in [
        "m := MAX_LIMITED(a := 1, b := 2, speed := 3);",
        "filter(x := 'text');",
        "filter(missing := 1.0);",
    ] {
        let errors = check_errors(&format!(
            "{MAX_LIMITED}{FILTER_EMA}PROGRAM Main\nVAR\n    m : DINT;\n    \
             filter : FILTER_EMA;\nEND_VAR\n{call}\nEND_PROGRAM\n"
        ));
        assert!(!errors.is_empty(), "{call}");
    }
}

#[test]
fn the_symbol_table_marks_a_rust_pou() {
    // RU-31: the symbol table marks each POU's language, which the tools read.
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        format!("{MAX_LIMITED}{FILTER_EMA}FUNCTION_BLOCK PLAIN\nEND_FUNCTION_BLOCK\n"),
    );
    let symbols = db.file_symbols(file);
    let rust = |name: &str| {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.modifiers.rust)
    };
    assert_eq!(rust("MAX_LIMITED"), Some(true));
    assert_eq!(rust("FILTER_EMA"), Some(true));
    assert_eq!(rust("PLAIN"), Some(false));
    // A variable of a Rust POU is no POU.
    assert_eq!(rust("alpha"), Some(false));
}

#[test]
fn a_statement_in_a_rust_pou_is_error_e310() {
    // RU-23 through the file's diagnostics: the rule's code and severity.
    let text = "{attribute 'rust'}\nFUNCTION_BLOCK F\nVAR\n    count : INT;\nEND_VAR\ncount := 1;\nEND_FUNCTION_BLOCK\n";
    assert_eq!(check_errors(text), vec![DiagnosticCode::InvalidRustPou]);
    assert_eq!(DiagnosticCode::InvalidRustPou.code(), "E310");
}

/// The interface I_COUNTER and RU-33's generated declaration of COUNTER, which implements it, with
/// the method lines `method`.
fn counter_implementing(method: &str) -> String {
    format!(
        "INTERFACE I_COUNTER
METHOD count_by : DINT
VAR_INPUT
    amount : DINT;
END_VAR
END_METHOD
END_INTERFACE

{{attribute 'rust'}}
FUNCTION_BLOCK COUNTER IMPLEMENTS I_COUNTER
VAR_OUTPUT
    count : DINT;
END_VAR
{method}END_FUNCTION_BLOCK
"
    )
}

#[test]
fn a_rust_blocks_interfaces_are_checked_against_its_methods() {
    // RU-33, IEC §6.6.6: a Rust block that implements an interface is checked as a Structured Text
    // block is, its body being Rust changes nothing there: its PUBLIC method `count_by` of the
    // prototype's signature conforms; without it, with an INT input where the prototype has DINT,
    // and PRIVATE, it is E208. Catches the check passing over a Rust POU's declaration as the
    // checks of a body do, which would let a Rust block claim an interface its methods do not give.
    let conforming =
        "METHOD PUBLIC count_by : DINT\nVAR_INPUT\n    amount : DINT;\nEND_VAR\nEND_METHOD\n";
    assert_eq!(
        check_errors(&counter_implementing(conforming)),
        Vec::<DiagnosticCode>::new()
    );
    for method in [
        "",
        "METHOD PUBLIC count_by : DINT\nVAR_INPUT\n    amount : INT;\nEND_VAR\nEND_METHOD\n",
        "METHOD PRIVATE count_by : DINT\nVAR_INPUT\n    amount : DINT;\nEND_VAR\nEND_METHOD\n",
    ] {
        assert_eq!(
            check_errors(&counter_implementing(method)),
            vec![DiagnosticCode::InterfaceConformance],
            "{method}"
        );
    }
    // An interface with a property is one a Rust block cannot implement: it has no properties
    // (RU-34).
    let with_property = counter_implementing(conforming).replace(
        "END_METHOD\nEND_INTERFACE",
        "END_METHOD\nPROPERTY level : DINT\nGET END_GET\nEND_PROPERTY\nEND_INTERFACE",
    );
    assert_eq!(
        check_errors(&with_property),
        vec![DiagnosticCode::InterfaceConformance]
    );
}
