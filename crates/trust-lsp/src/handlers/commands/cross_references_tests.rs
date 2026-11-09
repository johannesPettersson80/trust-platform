//! Specification 14 §6.3.2: `trust-lsp.crossReferences`.

use super::*;

/// Catches a client that gets references without their access or POU, a declaration not marked as
/// one, ranges in byte offsets instead of LSP positions, and an answer other than null for a
/// position on no symbol (inside a keyword, where no identifier is near).
#[test]
fn the_command_lists_a_globals_declaration_and_uses_with_access_and_pou() {
    let state = ServerState::new();
    let gvl = Url::parse("file:///project/src/GVL_IO.st").unwrap();
    let tank = Url::parse("file:///project/src/PRG_Tank.st").unwrap();
    state.open_document(
        gvl.clone(),
        1,
        "NAMESPACE GVL_IO\nVAR_GLOBAL\n    rLevelSP : REAL;\nEND_VAR\nEND_NAMESPACE\n".to_owned(),
    );
    state.open_document(
        tank.clone(),
        1,
        "PROGRAM PRG_Tank\nVAR\n    r : REAL;\nEND_VAR\nr := GVL_IO.rLevelSP;\nGVL_IO.rLevelSP := r;\nEND_PROGRAM\n".to_owned(),
    );
    let ask = |uri: &Url, line: u32, character: u32| {
        value(
            &state,
            vec![json!({ "uri": uri, "position": { "line": line, "character": character } })],
        )
        .expect("an answer")
    };
    let range = |line: u32, start: u32, end: u32| json!({ "start": { "line": line, "character": start }, "end": { "line": line, "character": end } });

    let answer = ask(&gvl, 2, 4);
    assert_eq!(
        answer,
        json!({
            "symbol": "rLevelSP",
            "references": [
                { "uri": gvl, "range": range(2, 4, 12), "access": "declaration", "pou": "GVL_IO" },
                { "uri": tank, "range": range(4, 12, 20), "access": "read", "pou": "PRG_Tank" },
                { "uri": tank, "range": range(5, 7, 15), "access": "write", "pou": "PRG_Tank" },
            ]
        })
    );
    assert_eq!(
        ask(&tank, 3, 2),
        Value::Null,
        "the middle of END_VAR names no symbol"
    );
}
