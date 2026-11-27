//! Completion after a namespace's name and `.` (specification 14 §6.1.2; IEC 61131-3 Ed.3
//! §6.9.3): the declarations of that namespace, from every file of the project, that code at the
//! position may name.

use super::*;

/// A library's namespaces: a block and a type in `Plant.Line1`, and an `INTERNAL` namespace
/// beside it.
const LIBRARY: &str = r#"
NAMESPACE Plant
NAMESPACE Line1
FUNCTION_BLOCK Motor
VAR_INPUT
    Run : BOOL;
END_VAR
END_FUNCTION_BLOCK

TYPE Mode : (Off, Auto);
END_TYPE
END_NAMESPACE

NAMESPACE INTERNAL Service
FUNCTION Reset : BOOL
Reset := TRUE;
END_FUNCTION
END_NAMESPACE
END_NAMESPACE
"#;

/// `Plant.Line1` continued in another file, which contributes to the same namespace (IEC
/// 61131-3 Ed.3 Table 64, NOTE).
const CONTINUATION: &str = r#"
NAMESPACE Plant.Line1
FUNCTION Start : BOOL
Start := TRUE;
END_FUNCTION
END_NAMESPACE
"#;

/// A CODESYS GVL: a namespace that holds global variables.
const GVL: &str = r#"
NAMESPACE GVL_IO
VAR_GLOBAL
    gLevel : INT;
    gPump : Plant.Line1.Motor;
END_VAR
END_NAMESPACE
"#;

/// The labels that completion offers at the `|` of `source`, a file of the project beside the
/// library, its continuation and the GVL, sorted.
fn labels_at(source: &str) -> Vec<String> {
    let cursor = source.find('|').expect("a cursor");
    let mut db = Database::new();
    for (file, text) in [(0, LIBRARY), (1, CONTINUATION), (2, GVL)] {
        db.set_source_text(FileId(file), text.to_string());
    }
    db.set_source_text(FileId(3), source.replacen('|', "", 1));
    let mut labels: Vec<String> = complete(&db, FileId(3), TextSize::from(cursor as u32))
        .into_iter()
        .map(|item| item.label.to_string())
        .collect();
    labels.sort();
    labels
}

/// Catches a namespace's name and dot that offer nothing (as they did until 27 November 2026),
/// that offer every name of the project, or that leave out what another file declares in the
/// namespace: `Plant.Line1`'s block, type and function, as a statement starts and inside an
/// expression.
#[test]
fn a_namespaces_name_and_dot_offer_its_declarations_of_every_file() {
    for source in [
        "PROGRAM Main\nPlant.Line1.|\nEND_PROGRAM\n",
        "PROGRAM Main\nVAR\n    Ok : BOOL;\nEND_VAR\nOk := Plant.Line1.|;\nEND_PROGRAM\n",
    ] {
        assert_eq!(labels_at(source), ["Mode", "Motor", "Start"], "{source}");
    }
}

/// Catches completion that falls back to every name of the project, or to none, while a
/// declaration's name is being typed after the namespace's dot (Codex's review of #842): the
/// editor asks again at each character and filters the same declarations by what was typed.
#[test]
fn a_namespaces_declarations_stay_while_one_is_typed() {
    let source =
        "PROGRAM Main\nVAR\n    Ok : BOOL;\nEND_VAR\nOk := Plant.Line1.Mo|;\nEND_PROGRAM\n";
    assert_eq!(labels_at(source), ["Mode", "Motor", "Start"]);
}

/// Catches an `INTERNAL` namespace, or what it holds, offered outside it, where nothing in it may
/// be named (specification 03 §4): `Plant.` offers only its public namespace, and
/// `Plant.Service.` nothing.
#[test]
fn an_internal_namespace_and_what_it_holds_are_not_offered_outside_it() {
    assert_eq!(labels_at("PROGRAM Main\nPlant.|\nEND_PROGRAM\n"), ["Line1"]);
    let contents = labels_at("PROGRAM Main\nPlant.Service.|\nEND_PROGRAM\n");
    assert_eq!(contents, Vec::<String>::new());
}

/// Catches an `INTERNAL` namespace whose declarations are left out inside it, where code may name
/// them: a function of `Plant.Service` is offered the library's `Reset`, and itself.
#[test]
fn an_internal_namespace_offers_what_it_holds_inside_it() {
    let source = r#"
NAMESPACE Plant
NAMESPACE INTERNAL Service
FUNCTION Check : BOOL
Check := Plant.Service.|;
END_FUNCTION
END_NAMESPACE
END_NAMESPACE
"#;
    assert_eq!(labels_at(source), ["Check", "Reset"]);
}

/// Catches a GVL's name and dot that offer nothing, and a global of the GVL whose members are
/// then lost: `GVL_IO.` offers its globals, and `GVL_IO.gPump.` the input of its block.
#[test]
fn a_gvls_name_and_dot_offer_its_globals_and_a_global_offers_its_members() {
    let globals = "PROGRAM Main\nGVL_IO.|\nEND_PROGRAM\n";
    assert_eq!(labels_at(globals), ["gLevel", "gPump"]);
    let members = "PROGRAM Main\nGVL_IO.gPump.|\nEND_PROGRAM\n";
    assert_eq!(labels_at(members), ["Run"]);
}

/// Catches a dotted name taken for the namespace when a local declaration has the namespace's
/// name: the local hides it (specification 09 §1.3), so `Plant.` offers the local's fields.
#[test]
fn a_local_named_like_a_namespace_hides_it() {
    let source = r#"
TYPE ST_Tank :
STRUCT
    Level : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    Plant : ST_Tank;
END_VAR
Plant.|
END_PROGRAM
"#;
    assert_eq!(labels_at(source), ["Level"]);
}
