//! A call through an interface variable as a call of each implementing method (specification 14 §6.3.2;
//! IEC 61131-3 Ed.3 §6.6.6): find references and the call hierarchy's incoming calls.

use super::*;
use text_size::TextRange;
use trust_ide::{incoming_calls, prepare_call_hierarchy};

const BLOCKS: &str = "INTERFACE I_Run
METHOD Run
END_METHOD
END_INTERFACE

INTERFACE I_RunFast EXTENDS I_Run
END_INTERFACE

FUNCTION_BLOCK FB_A IMPLEMENTS I_Run
METHOD Run
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_C IMPLEMENTS I_RunFast
METHOD Run
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Other
METHOD Run
END_METHOD
END_FUNCTION_BLOCK
";

const CALLS: &str = "PROGRAM P
VAR
    a : FB_A;
    other : FB_Other;
    itf : I_Run;
END_VAR
itf := a;
itf.Run();
a.Run();
other.Run();
END_PROGRAM
";

fn project() -> Database {
    let mut db = Database::new();
    db.set_source_text(FileId(0), BLOCKS.to_string());
    db.set_source_text(FileId(1), CALLS.to_string());
    db
}

/// The offset of the method name `Run` declared in the block `block`.
fn method_of(block: &str) -> usize {
    let start = BLOCKS
        .find(&format!("FUNCTION_BLOCK {block}"))
        .expect("the block");
    start + BLOCKS[start..].find("METHOD Run").expect("its method") + "METHOD ".len()
}

/// The range of `Run` in the call `call` of `CALLS`.
fn run_in(call: &str) -> TextRange {
    let at = CALLS.find(call).expect("the call") + call.find("Run").expect("Run");
    TextRange::at(TextSize::from(at as u32), TextSize::from(3))
}

fn references(at: usize) -> Vec<(u32, TextRange)> {
    let mut found: Vec<_> = find_references(
        &project(),
        FileId(0),
        TextSize::from(at as u32),
        FindReferencesOptions {
            include_declaration: false,
        },
    )
    .into_iter()
    .map(|reference| (reference.file_id.0, reference.range))
    .collect();
    found.sort_by_key(|(file, range)| (*file, range.start()));
    found
}

/// Specification 14 §6.3.2: `itf.Run()`, a call through a variable of `I_Run`, is a reference of each method that
/// implements `I_Run.Run`: of `FB_A.Run`, beside its direct call `a.Run()`, and of `FB_C.Run`, whose block implements
/// `I_RunFast`, which extends `I_Run`. `FB_Other.Run`, whose block implements no interface, keeps only `other.Run()`.
/// Catches the interface call missing from an implementing method's references, an interface reached through
/// another left out, and a same-named method of a block that implements nothing given the interface call.
#[test]
fn a_call_through_an_interface_is_a_reference_of_each_implementing_method() {
    assert_eq!(
        references(method_of("FB_A")),
        vec![(1, run_in("itf.Run()")), (1, run_in("a.Run()"))]
    );
    assert_eq!(
        references(method_of("FB_C")),
        vec![(1, run_in("itf.Run()"))]
    );
    assert_eq!(
        references(method_of("FB_Other")),
        vec![(1, run_in("other.Run()"))]
    );
}

/// Specification 14 §6.3.2: the incoming calls of `FB_C.Run`, whose block implements `I_RunFast`, which extends
/// `I_Run`, hold the program's call through a variable of `I_Run`, the only call it may receive. Catches the
/// interface call missing from the call hierarchy, and an interface reached through another left out.
#[test]
fn the_incoming_calls_of_an_implementing_method_hold_its_calls_through_an_interface() {
    let db = project();
    let item = prepare_call_hierarchy(&db, FileId(0), TextSize::from(method_of("FB_C") as u32))
        .expect("the method is a call hierarchy item");
    let calls = incoming_calls(&db, &item);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].from.name.as_str(), "P");
    let texts: Vec<&str> = calls[0]
        .from_ranges
        .iter()
        .map(|range| CALLS[usize::from(range.start())..usize::from(range.end())].trim())
        .collect();
    assert_eq!(texts, ["itf.Run()"]);
}
