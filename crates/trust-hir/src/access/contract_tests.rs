//! Specification 28 §6: the access of each use, a read, a write or a call.

use super::*;
use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use text_size::TextSize;

const SOURCE: &str = "FUNCTION Bump : BOOL\nVAR_IN_OUT\n    v : INT;\nEND_VAR\nBump := TRUE;\nEND_FUNCTION\nFUNCTION_BLOCK FB_Tank\nMETHOD PUBLIC Start\nEND_METHOD\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR\n    tank : FB_Tank;\n    timer : TON;\n    done : BOOL;\n    count : INT;\n    table : ARRAY[0..3] OF INT;\n    slot : INT;\n    pass : INT;\n    ok : BOOL;\nEND_VAR\nok := Bump(count);\ntimer(IN := ok, PT := T#1s, Q => done);\ntank.Start();\ntable[slot] := count;\nFOR pass := 0 TO 3 DO\n    ok := done;\nEND_FOR;\nEND_PROGRAM\n";

/// The access of the use at the `n`-th occurrence of `text` in the program's body; a member is its
/// member access (`tank.Start`).
fn access_at(text: &str, n: usize) -> Access {
    let mut db = Database::new();
    db.set_source_text(FileId(0), SOURCE.to_owned());
    let analysis = db.analyze(FileId(0));
    let root = trust_syntax::parser::parse(SOURCE).syntax();
    let body = SOURCE.find("ok := Bump").unwrap();
    let offset = body + SOURCE[body..].match_indices(text).nth(n).unwrap().0;
    let token = root
        .token_at_offset(TextSize::from(offset as u32))
        .right_biased()
        .unwrap();
    let mut node = token.parent().unwrap();
    if node.kind() == SyntaxKind::Name
        && let Some(member) = node
            .parent()
            .filter(|parent| parent.kind() == SyntaxKind::FieldExpr)
    {
        node = member;
    }
    access_of(&node, &analysis.written_arguments)
}

/// Catches each wrong kind: an in-out argument or an output binding read (the binder's record not
/// reaching the rule), an input argument written, a called instance or method read, an index
/// written with its array, an assignment's value written, and a FOR loop's control variable read.
#[test]
fn each_use_has_its_access() {
    assert_eq!(access_at("ok", 0), Access::Write, "an assignment's target");
    assert_eq!(access_at("Bump", 0), Access::Call, "a called function");
    assert_eq!(
        access_at("count", 0),
        Access::Write,
        "an in-out argument by position"
    );
    assert_eq!(access_at("timer", 0), Access::Call, "a called instance");
    assert_eq!(access_at("ok", 1), Access::Read, "an input argument");
    assert_eq!(access_at("done", 0), Access::Write, "an output binding");
    assert_eq!(
        access_at("tank", 0),
        Access::Call,
        "the instance of a called method"
    );
    assert_eq!(access_at("Start", 0), Access::Call, "a called method");
    assert_eq!(
        access_at("table", 0),
        Access::Write,
        "an array whose element is assigned"
    );
    assert_eq!(access_at("slot", 0), Access::Read, "an index");
    assert_eq!(access_at("count", 1), Access::Read, "an assignment's value");
    assert_eq!(
        access_at("pass", 0),
        Access::Write,
        "a FOR loop's control variable"
    );
    assert_eq!(access_at("done", 1), Access::Read, "a read in the loop");
}
