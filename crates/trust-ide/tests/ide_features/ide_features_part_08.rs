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
METHOD PUBLIC Run
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_C IMPLEMENTS I_RunFast
METHOD PUBLIC Run
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Other
METHOD PUBLIC Run
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
    start
        + BLOCKS[start..]
            .find("METHOD PUBLIC Run")
            .expect("its method")
        + "METHOD PUBLIC ".len()
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

/// Specification 14 §6.3.2: the incoming calls of `FB_A.Run` hold the program's call through a variable of `I_Run`
/// beside its direct call, and those of `FB_C.Run`, whose block implements `I_RunFast`, which extends `I_Run`, hold
/// the interface call alone. Catches the interface call missing from the call hierarchy, and an interface reached
/// through another left out.
#[test]
fn the_incoming_calls_of_an_implementing_method_hold_its_calls_through_an_interface() {
    let db = project();
    let calls_of = |block: &str| {
        let item = prepare_call_hierarchy(&db, FileId(0), TextSize::from(method_of(block) as u32))
            .expect("the method is a call hierarchy item");
        let calls = incoming_calls(&db, &item);
        assert_eq!(calls.len(), 1, "{block}: {calls:?}");
        assert_eq!(calls[0].from.name.as_str(), "P");
        let mut ranges = calls[0].from_ranges.clone();
        ranges.sort_by_key(|range| range.start());
        ranges
            .iter()
            .map(|range| {
                CALLS[usize::from(range.start())..usize::from(range.end())]
                    .trim()
                    .to_string()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(calls_of("FB_A"), ["itf.Run()", "a.Run()"]);
    assert_eq!(calls_of("FB_C"), ["itf.Run()"]);
}

/// Specification 04, "Interface Inheritance": the editor shows every base of an interface that
/// extends several, as its type hierarchy's supertypes and in its hover, and completes the methods
/// that each base gives on a value of it. Catches the editor reading only the first base.
#[test]
fn test_an_interface_with_several_bases_shows_and_completes_each() {
    let source = r#"
INTERFACE IStart
    METHOD Start : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IStop
    METHOD Stop : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IMotor EXTENDS IStart, IStop
END_INTERFACE

PROGRAM Main
VAR
    itf : IMotor;
END_VAR
    itf.|
END_PROGRAM
"#;
    let cursor = source.find('|').expect("cursor");
    let mut cleaned = source.to_string();
    cleaned.remove(cursor);
    let (db, file) = setup(&cleaned);

    let motor = TextSize::from(cleaned.find("IMotor EXTENDS").expect("IMotor") as u32);
    let item = trust_ide::prepare_type_hierarchy(&db, file, motor).expect("type hierarchy item");
    let bases: Vec<String> = trust_ide::supertypes(&db, &item)
        .iter()
        .map(|base| base.name.to_string())
        .collect();
    assert_eq!(bases, vec!["IStart".to_string(), "IStop".to_string()]);

    let motor_hover = hover(&db, file, motor).expect("hover");
    assert!(
        motor_hover.contents.contains("EXTENDS IStart, IStop"),
        "{}",
        motor_hover.contents
    );

    let labels: Vec<String> = complete(&db, file, TextSize::from(cursor as u32))
        .iter()
        .map(|item| item.label.to_string())
        .collect();
    for member in ["Start", "Stop"] {
        assert!(
            labels.iter().any(|label| label == member),
            "{member} missing from {labels:?}"
        );
    }
}
