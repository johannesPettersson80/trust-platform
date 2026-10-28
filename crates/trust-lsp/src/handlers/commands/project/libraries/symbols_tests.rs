//! A library's symbols in the Libraries panel (specification 25 §7: search, declaration detail,
//! copy and insertion). The first case is the extension's former `libraries-model.test.ts`
//! assertion, moved here with the rule.

use super::*;

fn summary(text: &str) -> Vec<(String, String, String, String)> {
    summarize(&[("lib.st".to_owned(), text.to_owned())])
        .into_iter()
        .map(|symbol| {
            (
                symbol.kind.to_owned(),
                symbol.name,
                symbol.declaration,
                symbol.snippet,
            )
        })
        .collect()
}

#[test]
fn symbols_are_grouped_by_kind_with_their_declaring_line() {
    let found = summary(
        "TYPE AxisState : INT; END_TYPE\nFUNCTION SCALE : REAL\nEND_FUNCTION\nFUNCTION_BLOCK Averager\nEND_FUNCTION_BLOCK\n",
    );
    let kinds_and_names: Vec<_> = found
        .iter()
        .map(|(kind, name, _, _)| (kind.as_str(), name.as_str()))
        .collect();
    assert_eq!(
        kinds_and_names,
        [
            ("function", "SCALE"),
            ("function_block", "Averager"),
            ("type", "AxisState")
        ]
    );
    let declarations: Vec<_> = found.iter().map(|(_, _, line, _)| line.as_str()).collect();
    assert_eq!(
        declarations,
        [
            "FUNCTION SCALE : REAL",
            "FUNCTION_BLOCK Averager",
            "TYPE AxisState : INT; END_TYPE"
        ]
    );
}

#[test]
fn a_declaration_after_a_blank_line_keeps_its_own_line() {
    // The keyword's own line is the declaration, also after an empty line and with indentation and
    // repeated blanks. Catches: the empty line before it shown as the declaration (the former
    // pattern started its match on the blank line).
    let found = summary("END_TYPE\n\n   FUNCTION_BLOCK   Motor  // drives\n");
    assert_eq!(found[0].1, "Motor");
    assert_eq!(found[0].2, "FUNCTION_BLOCK Motor // drives");
}

#[test]
fn only_a_keyword_at_a_line_start_followed_by_a_blank_declares() {
    // `END_FUNCTION`, `MY_TYPE x` and `FUNCTIONX` declare nothing; the keyword's letter case does not
    // matter. Catches: a match inside a word or after other text on the line.
    let found =
        summary("END_FUNCTION Foo\nx := MY_TYPE Bar;\nFUNCTIONX Baz\nfunction_block Lower\n");
    let names: Vec<_> = found.iter().map(|(_, name, _, _)| name.as_str()).collect();
    assert_eq!(names, ["Lower"]);
}

#[test]
fn each_kind_and_name_is_listed_once_from_the_first_file() {
    // The same function in two files: the first file's declaration stays. Catches: duplicates in the
    // symbol browser's count.
    let symbols = summarize(&[
        ("a.st".to_owned(), "FUNCTION Twice : INT\n".to_owned()),
        ("b.st".to_owned(), "FUNCTION Twice : DINT\n".to_owned()),
    ]);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].file, "a.st");
    assert_eq!(symbols[0].declaration, "FUNCTION Twice : INT");
}

#[test]
fn names_are_ordered_as_a_person_reads_them() {
    // Letter case counts last (small first) and `_` sorts before digits and letters. Catches: a byte
    // order, which puts every capital before every small letter.
    let names: Vec<_> = summarize(&[(
        "lib.st".to_owned(),
        "FUNCTION beta : INT\nFUNCTION Alpha : INT\nFUNCTION A_1 : INT\nFUNCTION A1 : INT\nFUNCTION alpha : INT\n".to_owned(),
    )])
    .into_iter()
    .map(|symbol| symbol.name)
    .collect();
    assert_eq!(names, ["A_1", "A1", "alpha", "Alpha", "beta"]);
}

#[test]
fn snippets_declare_an_instance_a_value_or_a_call() {
    // `<instance> : <FB>;` with the instance name's first letter small, `value : <TYPE>;` and
    // `<FUNCTION>(...)`, as the panel inserts them. Catches: an insertion that does not compile.
    assert_eq!(
        snippet("function_block", "Averager"),
        "averager : Averager;"
    );
    assert_eq!(
        snippet("function_block", "MC_Power"),
        "mC_Power : MC_Power;"
    );
    assert_eq!(snippet("type", "AxisState"), "value : AxisState;");
    assert_eq!(snippet("function", "SCALE"), "SCALE(...)");
}

#[test]
fn library_symbols_read_st_and_pou_files_below_the_folder() {
    // `.ST` and `.Pou` count (any letter case), `.txt` does not, and subfolders are read. Catches: a
    // scan of the top folder only.
    let root =
        std::env::temp_dir().join(format!("trust-lsp-library-symbols-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/deep")).expect("folders");
    std::fs::write(root.join("src/A.ST"), "FUNCTION One : INT\n").expect("file");
    std::fs::write(root.join("src/deep/b.Pou"), "FUNCTION_BLOCK Two\n").expect("file");
    std::fs::write(root.join("src/notes.txt"), "FUNCTION Three : INT\n").expect("file");
    let files = library_sources(&root);
    let names: Vec<_> = library_symbols(&files)
        .into_iter()
        .map(|symbol| symbol.name)
        .collect();
    let first = files.first().cloned();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(names, ["One", "Two"]);
    assert_eq!(
        first,
        Some(root.join("src/A.ST")),
        "the first source in path order"
    );
}
