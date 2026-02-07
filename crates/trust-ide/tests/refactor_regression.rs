use expect_test::expect;
use text_size::{TextRange, TextSize};

use trust_hir::db::{FileId, SourceDatabase};
use trust_hir::Database;
use trust_ide::refactor::parse_namespace_path;
use trust_ide::rename::RenameResult;
use trust_ide::{
    convert_function_block_to_function, convert_function_to_function_block, extract_method,
    generate_interface_stubs, inline_symbol, move_namespace_path,
};

fn format_edits(result: &RenameResult) -> String {
    let mut files: Vec<_> = result.edits.iter().collect();
    files.sort_by_key(|(file_id, _)| file_id.0);

    let mut out = String::new();
    for (file_id, edits) in files {
        out.push_str(&format!("file {}:\n", file_id.0));

        let mut sorted = edits.clone();
        sorted.sort_by(|a, b| {
            let ak = (
                u32::from(a.range.start()),
                u32::from(a.range.end()),
                a.new_text.as_str(),
            );
            let bk = (
                u32::from(b.range.start()),
                u32::from(b.range.end()),
                b.new_text.as_str(),
            );
            ak.cmp(&bk)
        });

        for edit in sorted {
            let start = u32::from(edit.range.start());
            let end = u32::from(edit.range.end());
            let escaped: String = edit
                .new_text
                .chars()
                .flat_map(char::escape_default)
                .collect();
            out.push_str(&format!("  [{start}..{end}] => \"{escaped}\"\n"));
        }
    }

    out
}

#[test]
fn refactor_move_namespace_edit_snapshot() {
    let source = r#"
NAMESPACE LibA
TYPE Foo : INT;
END_TYPE
FUNCTION FooFunc : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
    USING LibA;
    VAR
        x : LibA.Foo;
    END_VAR
    x := LibA.FooFunc();
END_PROGRAM
"#;

    let mut db = Database::new();
    let file_id = FileId(0);
    db.set_source_text(file_id, source.to_string());

    let old_path = parse_namespace_path("LibA").expect("old path");
    let new_path = parse_namespace_path("Company.LibA").expect("new path");

    let result = move_namespace_path(&db, &old_path, &new_path).expect("move namespace");
    let snapshot = format_edits(&result);
    expect![[r#"
        file 0:
          [11..15] => "Company.LibA"
          [115..119] => "Company.LibA"
          [141..149] => "Company.LibA.Foo"
          [172..184] => "Company.LibA.FooFunc"
    "#]]
    .assert_eq(&snapshot);
}

#[test]
fn refactor_generate_stubs_edit_snapshot() {
    let source = r#"
INTERFACE IControl
    METHOD Start
    END_METHOD

    PROPERTY Status : INT
        GET
        END_GET
    END_PROPERTY
END_INTERFACE

CLASS Pump IMPLEMENTS IControl
END_CLASS
"#;

    let mut db = Database::new();
    let file_id = FileId(0);
    db.set_source_text(file_id, source.to_string());

    let offset = source.find("IMPLEMENTS IControl").expect("implements");
    let result = generate_interface_stubs(&db, file_id, TextSize::from(offset as u32))
        .expect("generate interface stubs");
    let snapshot = format_edits(&result);
    expect![[r#"
        file 0:
          [170..170] => "    METHOD PUBLIC Start\n        // TODO: implement\n    END_METHOD\n\n    PROPERTY PUBLIC Status : INT\n    GET\n        // TODO: implement\n    END_GET\n    END_PROPERTY\n"
    "#]].assert_eq(&snapshot);
}

#[test]
fn refactor_inline_symbol_edit_snapshot() {
    let source = r#"
PROGRAM Test
    VAR
        x : INT := 1 + 2;
    END_VAR
    y := x;
END_PROGRAM
"#;

    let mut db = Database::new();
    let file_id = FileId(0);
    db.set_source_text(file_id, source.to_string());

    let offset = source.find("x;").expect("reference");
    let result = inline_symbol(&db, file_id, TextSize::from(offset as u32)).expect("inline symbol");
    let snapshot = format_edits(&result.edits);
    expect![[r#"
        file 0:
          [21..60] => ""
          [69..70] => "(1 + 2)"
    "#]]
    .assert_eq(&snapshot);
}

#[test]
fn refactor_extract_method_edit_snapshot() {
    let source = r#"
CLASS Controller
    METHOD Run
        VAR
            x : INT;
            y : INT;
        END_VAR
        x := 1;
        y := x + 1;
    END_METHOD
END_CLASS
"#;

    let mut db = Database::new();
    let file_id = FileId(0);
    db.set_source_text(file_id, source.to_string());

    let start = source.find("x := 1;").expect("start");
    let end = source.find("y := x + 1;").expect("end") + "y := x + 1;".len();
    let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

    let result = extract_method(&db, file_id, range).expect("extract method");
    let snapshot = format_edits(&result.edits);
    expect![[r#"
        file 0:
          [111..143] => "        ExtractedMethod(x := x, y := y);"
          [154..154] => "\n    METHOD ExtractedMethod\n    VAR_IN_OUT\n        x : INT;\n        y : INT;\n    END_VAR\n        x := 1;\n        y := x + 1;\n    END_METHOD\n"
    "#]].assert_eq(&snapshot);
}

#[test]
fn refactor_convert_function_to_function_block_edit_snapshot() {
    let source = r#"
FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION

PROGRAM Main
    Foo();
END_PROGRAM
"#;

    let mut db = Database::new();
    let file_id = FileId(0);
    db.set_source_text(file_id, source.to_string());

    let offset = source.find("FUNCTION Foo").expect("function");
    let result = convert_function_to_function_block(&db, file_id, TextSize::from(offset as u32))
        .expect("convert function to function block");
    let snapshot = format_edits(&result);
    expect![[r#"
        file 0:
          [1..9] => "FUNCTION_BLOCK"
          [14..24] => ""
          [24..24] => "    VAR_OUTPUT\n        result : INT;\n    END_VAR"
          [24..27] => "result"
          [34..46] => "END_FUNCTION_BLOCK"
          [65..65] => "\n\n    VAR\n        FooInstance : Foo;\n    END_VAR\n"
          [65..68] => "FooInstance"
    "#]]
    .assert_eq(&snapshot);
}

#[test]
fn refactor_convert_function_block_to_function_edit_snapshot() {
    let source = r#"
FUNCTION_BLOCK Fb
    VAR_OUTPUT
        result : INT;
    END_VAR
    result := 1;
END_FUNCTION_BLOCK
"#;

    let mut db = Database::new();
    let file_id = FileId(0);
    db.set_source_text(file_id, source.to_string());

    let offset = source.find("FUNCTION_BLOCK Fb").expect("function block");
    let result = convert_function_block_to_function(&db, file_id, TextSize::from(offset as u32))
        .expect("convert function block to function");
    let snapshot = format_edits(&result);
    expect![[r#"
        file 0:
          [1..15] => "FUNCTION"
          [23..23] => " : INT"
          [23..85] => ""
          [85..103] => "END_FUNCTION"
    "#]]
    .assert_eq(&snapshot);
}
