#[cfg(test)]
mod tests {
    use super::*;
    use trust_hir::db::{Database, FileId, SourceDatabase};

    #[test]
    fn test_top_level_keywords() {
        let items = top_level_keywords();
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "FUNCTION_BLOCK"));
    }

    #[test]
    fn test_type_keywords() {
        let items = type_keywords();
        assert!(items.iter().any(|i| i.label == "INT"));
        assert!(items.iter().any(|i| i.label == "BOOL"));
    }

    #[test]
    fn test_parameter_name_completion_in_call() {
        let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(|);
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        assert!(items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("A")));
        assert!(items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("B")));
        let a_item = items
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case("A"))
            .expect("A completion");
        let insert = a_item.insert_text.as_ref().expect("insert text");
        assert!(insert.contains("A"));
        assert!(insert.contains(":="));
    }

    #[test]
    fn test_parameter_name_completion_skips_used_formal() {
        let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(A := 1, |);
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        assert!(!items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("A")));
        assert!(items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("B")));
    }

    #[test]
    fn test_parameter_name_completion_in_method_call() {
        let source = r#"
FUNCTION_BLOCK Motor
METHOD PUBLIC Start : BOOL
VAR_INPUT
    Var1 : BOOL;
    Var2 : BOOL;
END_VAR
    Start := Var1 AND Var2;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    motor : Motor;
    result : BOOL;
END_VAR
    result := motor.Start(|);
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        assert!(items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("Var1")));
        assert!(items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("Var2")));

        let var1 = items
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case("Var1"))
            .expect("Var1 completion");
        assert_eq!(var1.kind, CompletionKind::Variable);
        assert!(
            var1.insert_text
                .as_ref()
                .is_some_and(|insert| insert.contains("Var1 := ")),
            "expected formal assignment insert text for method input"
        );
    }

    #[test]
    fn test_standard_function_completion() {
        let source = r#"
PROGRAM Main
VAR
    x : INT;
END_VAR
    x := |;
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        let abs_item = items
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case("ABS"))
            .expect("ABS completion");
        assert!(abs_item
            .documentation
            .as_ref()
            .map(|doc| doc.contains("IEC 61131-3"))
            .unwrap_or(false));
    }

    #[test]
    fn test_typed_literal_completion() {
        let source = r#"
PROGRAM Main
VAR
    x : TIME;
END_VAR
    x := |;
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        assert!(items.iter().any(|item| item.label == "T#1s"));
        assert!(items.iter().any(|item| item.label == "DATE#2024-01-15"));
    }

    #[test]
    fn test_typed_literal_completion_after_prefix() {
        let source = r#"
PROGRAM Main
VAR
    x : TIME;
END_VAR
    x := T#|;
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        let item = items
            .iter()
            .find(|item| item.label == "T#1s")
            .expect("typed literal snippet");
        let edit = item.text_edit.as_ref().expect("text edit");
        assert_eq!(edit.new_text.as_str(), "${1:1s}");
        assert_eq!(edit.range.start(), TextSize::from(cursor as u32));
        assert_eq!(edit.range.end(), TextSize::from(cursor as u32));
    }

    #[test]
    fn test_member_completion_respects_visibility() {
        let source = r#"
INTERFACE ICounter
    METHOD Next : DINT
    END_METHOD
    PROPERTY Value : DINT
        GET
        END_GET
    END_PROPERTY
END_INTERFACE

FUNCTION_BLOCK CounterFb IMPLEMENTS ICounter
VAR
    x : DINT;
END_VAR

METHOD PUBLIC Next : DINT
    x := x + 1;
    Next := x;
END_METHOD

PUBLIC PROPERTY Value : DINT
    GET
        Value := x;
    END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    counter : CounterFb;
END_VAR
    counter.|
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        assert!(items.iter().any(|item| item.label == "Next"));
        assert!(items.iter().any(|item| item.label == "Value"));
        assert!(!items.iter().any(|item| item.label == "x"));
    }

    #[test]
    fn test_completion_recovery_in_statement_context_keeps_scope_symbols() {
        let source = r#"
PROGRAM PlantProgram
VAR
    Pump : FB_Pump;
    Cmd : ST_PumpCommand;
    Status : ST_PumpStatus;
    HaltReq : BOOL;
END_VAR

Sta|
Pump(Command := Cmd);
Status := Pump.Status;
HaltReq := FALSE;
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        assert!(
            items
                .iter()
                .any(|item| item.label.eq_ignore_ascii_case("Status")),
            "completion should include local variable Status in recovered statement context"
        );
        assert!(
            items
                .iter()
                .any(|item| item.label.eq_ignore_ascii_case("Cmd")),
            "completion should include local variable Cmd in recovered statement context"
        );
        assert!(
            items
                .iter()
                .any(|item| item.label.eq_ignore_ascii_case("Pump")),
            "completion should include local variable Pump in recovered statement context"
        );
        assert!(
            items
                .iter()
                .any(|item| item.label.eq_ignore_ascii_case("HaltReq")),
            "completion should include local variable HaltReq in recovered statement context"
        );
    }

    #[test]
    fn test_using_namespace_completion_info() {
        let source = r#"
NAMESPACE Lib
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
USING Lib;
VAR
    x : INT;
END_VAR
    x := |;
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        let foo = items
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case("Foo"))
            .expect("Foo completion");
        assert!(foo
            .documentation
            .as_ref()
            .map(|doc| doc.contains("USING Lib"))
            .unwrap_or(false));
    }
}
