//! Specification 14 §6.3.2: the cross-reference list of a symbol.

use super::*;
use trust_hir::db::FileId;

/// An entry as (file, offset, text, kind, POU).
type Entry = (u32, usize, String, CrossReferenceKind, Option<String>);

/// The list at `at` in file `file`, as (file, offset, text, kind, POU) per entry.
fn list(sources: &[&str], file: u32, at: usize) -> (SmolStr, Vec<Entry>) {
    let mut db = Database::new();
    for (index, source) in sources.iter().enumerate() {
        db.set_source_text(FileId(index as u32), (*source).to_owned());
    }
    let (name, entries) =
        cross_references(&db, FileId(file), TextSize::from(at as u32)).expect("a symbol");
    let entries = entries
        .into_iter()
        .map(|entry| {
            let source = sources[entry.file_id.0 as usize];
            let start = usize::from(entry.range.start());
            let text = source[start..usize::from(entry.range.end())].to_owned();
            (entry.file_id.0, start, text, entry.kind, entry.pou)
        })
        .collect();
    (name, entries)
}

const GVL: &str = "NAMESPACE GVL_IO\nVAR_GLOBAL\n    gLevel : INT;\n    gTank : FB_Tank;\nEND_VAR\nEND_NAMESPACE\n";
const LIB: &str = "FUNCTION Bump : BOOL\nVAR_IN_OUT\n    v : INT;\nEND_VAR\nv := v + 1;\nBump := TRUE;\nEND_FUNCTION\nFUNCTION_BLOCK FB_Tank\nMETHOD PUBLIC Start\nEND_METHOD\nEND_FUNCTION_BLOCK\n";
const TANK: &str = "PROGRAM PRG_Tank\nVAR\n    ok : BOOL;\n    copy : INT;\nEND_VAR\ncopy := GVL_IO.gLevel;\nGVL_IO.gLevel := copy;\nok := Bump(GVL_IO.gLevel);\nGVL_IO.gTank.Start();\nEND_PROGRAM\n";

/// The offset of `name` at the end of `qualified`'s first occurrence in `source`.
fn at(source: &str, qualified: &str, name: &str) -> usize {
    source.find(qualified).unwrap() + qualified.len() - name.len()
}

/// Catches a list without the declaration, a use with the wrong access (a read for an in-out
/// argument, a read for the instance of a called method), a POU path without its namespace or
/// method, and entries out of file and offset order.
#[test]
fn a_symbols_list_holds_its_declaration_and_each_use_with_access_and_pou() {
    let gvl_pou = Some("GVL_IO".to_owned());
    let tank_pou = Some("PRG_Tank".to_owned());
    let declared = GVL.find("gLevel").unwrap();
    let (name, entries) = list(&[GVL, LIB, TANK], 0, declared);
    assert_eq!(name, "gLevel");
    assert_eq!(
        entries,
        vec![
            (
                0,
                declared,
                "gLevel".to_owned(),
                CrossReferenceKind::Declaration,
                gvl_pou.clone()
            ),
            (
                2,
                at(TANK, "copy := GVL_IO.gLevel", "gLevel"),
                "gLevel".to_owned(),
                CrossReferenceKind::Use(Access::Read),
                tank_pou.clone()
            ),
            (
                2,
                at(TANK, "\nGVL_IO.gLevel := copy", "gLevel := copy"),
                "gLevel".to_owned(),
                CrossReferenceKind::Use(Access::Write),
                tank_pou.clone()
            ),
            (
                2,
                at(TANK, "Bump(GVL_IO.gLevel", "gLevel"),
                "gLevel".to_owned(),
                CrossReferenceKind::Use(Access::Write),
                tank_pou.clone()
            ),
        ]
    );

    let method = LIB.find("Start").unwrap();
    let (name, entries) = list(&[GVL, LIB, TANK], 1, method);
    assert_eq!(name, "Start");
    assert_eq!(
        entries,
        vec![
            (
                1,
                method,
                "Start".to_owned(),
                CrossReferenceKind::Declaration,
                Some("FB_Tank.Start".to_owned())
            ),
            (
                2,
                at(TANK, "GVL_IO.gTank.Start", "Start"),
                "Start".to_owned(),
                CrossReferenceKind::Use(Access::Call),
                tank_pou.clone()
            ),
        ]
    );

    let (_, entries) = list(&[GVL, LIB, TANK], 0, GVL.find("gTank").unwrap());
    assert_eq!(
        entries.iter().map(|entry| entry.3).collect::<Vec<_>>(),
        [
            CrossReferenceKind::Declaration,
            CrossReferenceKind::Use(Access::Call)
        ]
    );
}
