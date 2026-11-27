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
        let validate_item = items
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case("IS_VALID_BCD"))
            .expect("IS_VALID_BCD completion");
        assert!(validate_item
            .documentation
            .as_ref()
            .map(|doc| doc.contains("Table 39"))
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

        // Under the CODESYS profile the plain VAR member is public (IEC_DEVIATIONS DEV-028), so
        // it is offered beside the methods and properties.
        db.set_dialect(trust_hir::Dialect::Codesys);
        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        assert!(items.iter().any(|item| item.label == "x"));
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

    // A namespace name completes in a statement under its own kind. Catches the namespace
    // listed as a variable, which VS Code showed with the variable icon.
    #[test]
    fn namespace_completes_as_a_namespace() {
        let source = r#"
NAMESPACE Control
FUNCTION Clamp : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
VAR
    Level : INT;
END_VAR
    Level := |;
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let items = complete(&db, file_id, TextSize::from(cursor as u32));
        let control = items
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case("Control"))
            .expect("Control completion");
        assert_eq!(control.kind, CompletionKind::Namespace);
    }

    /// The labels that completion offers at the `|` of `source`.
    fn labels_at(source: &str) -> Vec<String> {
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);
        complete(&db, file_id, TextSize::from(cursor as u32))
            .into_iter()
            .map(|item| item.label.to_string())
            .collect()
    }

    /// Specification 14 §6.1.2: completion does not offer what CODESYS hides from its Input Assistant: a global with
    /// `{attribute 'hide'}` above its line, a POU with it in its first line, and the `VAR` locals of a POU with
    /// `{attribute 'hide_all_locals'}`; the declarations beside them are offered. Catches a hidden name offered, and a
    /// visible name beside it dropped.
    #[test]
    fn completion_leaves_out_what_the_hide_attributes_hide() {
        let labels = labels_at(
            r#"
VAR_GLOBAL
    {attribute 'hide'}
    gSecret : INT;
    gShown : INT;
END_VAR

{attribute 'hide'}
FUNCTION HiddenHelper : INT
    HiddenHelper := 1;
END_FUNCTION

FUNCTION ShownHelper : INT
    ShownHelper := 2;
END_FUNCTION

{attribute 'hide_all_locals'}
PROGRAM Main
VAR_INPUT
    Request : INT;
END_VAR
VAR
    scratch : INT;
END_VAR
    scratch := |;
END_PROGRAM
"#,
        );
        for shown in ["gShown", "ShownHelper", "Request"] {
            assert!(labels.iter().any(|label| label == shown), "{shown} missing: {labels:?}");
        }
        for hidden in ["gSecret", "HiddenHelper", "scratch"] {
            assert!(!labels.iter().any(|label| label == hidden), "{hidden} offered: {labels:?}");
        }
    }

    /// The labels offered at `|` in `source`, sorted.
    fn sorted_labels_at(source: &str) -> Vec<String> {
        let mut labels = labels_at(source);
        labels.sort();
        labels
    }

    /// A function block declaring a member of every kind, for the member completion tests below.
    const VALVE: &str = r#"
FUNCTION_BLOCK FB_Valve
VAR_INPUT
    Open : BOOL;
END_VAR
VAR_OUTPUT
    IsOpen : BOOL;
END_VAR
VAR_IN_OUT
    Counter : INT;
END_VAR
VAR
    travel : TIME;
END_VAR
VAR PUBLIC
    Cycles : DINT;
END_VAR
METHOD PUBLIC Reset
END_METHOD
END_FUNCTION_BLOCK
"#;

    /// Specification 14 §6.1.2: after `.` on an instance of the standard TON, completion offers its
    /// inputs IN and PT and its outputs Q and ET (IEC 61131-3 Ed.3 Table 45), which code outside the
    /// block may assign and read (§6.6.3.4.2, Figure 13). Catches member completion that keeps only
    /// variables, methods and properties, which answered nothing for a TON.
    #[test]
    fn member_completion_offers_a_standard_blocks_inputs_and_outputs() {
        let source = r#"
PROGRAM Main
VAR
    Timer : TON;
END_VAR
    Timer.|
END_PROGRAM
"#;
        assert_eq!(sorted_labels_at(source), ["ET", "IN", "PT", "Q"]);
    }

    /// Specification 14 §6.1.2: outside a function block, `.` on its instance offers its inputs,
    /// outputs, public variables and methods, and neither its in-out, which IEC 61131-3 Ed.3
    /// §6.6.3.4.2 Figure 13 does not allow there, nor its protected internal variable. Catches an
    /// input or output left out, and an in-out or a protected variable offered outside.
    #[test]
    fn member_completion_outside_a_block_offers_what_code_there_may_name() {
        let source = format!(
            "{VALVE}
PROGRAM Main
VAR
    valve : FB_Valve;
END_VAR
    valve.|
END_PROGRAM
"
        );
        assert_eq!(sorted_labels_at(&source), ["Cycles", "IsOpen", "Open", "Reset"]);
    }

    /// Specification 14 §6.1.2: in the body of a block derived from FB_Valve, `THIS^.` offers the
    /// base's in-out and protected variable too, as the analysis accepts them there. Catches member
    /// completion that drops a protected member where it may be named.
    #[test]
    fn member_completion_in_a_derived_block_offers_its_protected_members_too() {
        let source = format!(
            "{VALVE}
FUNCTION_BLOCK FB_FastValve EXTENDS FB_Valve
    THIS^.|
END_FUNCTION_BLOCK
"
        );
        assert_eq!(
            sorted_labels_at(&source),
            ["Counter", "Cycles", "IsOpen", "Open", "Reset", "travel"]
        );
    }

    /// Specification 14 §6.1.2: in a method of FB_Valve itself, `THIS^.` leaves out the block's in-out,
    /// which belongs to its body and which the analysis rejects in its methods (Codex's review of
    /// #842), and offers the rest, the protected variable among them. Catches the in-out offered.
    #[test]
    fn member_completion_in_a_method_of_the_block_leaves_out_its_in_outs() {
        let source = r#"
FUNCTION_BLOCK FB_Valve
VAR_INPUT
    Open : BOOL;
END_VAR
VAR_IN_OUT
    Counter : INT;
END_VAR
VAR
    travel : TIME;
END_VAR
METHOD PUBLIC Reset
    THIS^.|
END_METHOD
END_FUNCTION_BLOCK
"#;
        assert_eq!(sorted_labels_at(source), ["Open", "Reset", "travel"]);
    }

    /// Specification 14 §6.1.1: completion asked again while a member's name is typed after the dot
    /// (`Timer.E`, as VS Code asks while the user types) offers the members, not the statement list.
    /// Catches the context taken from the token after the cursor, which answered keywords such as
    /// PROGRAM and FUNCTION and every name in scope.
    #[test]
    fn member_completion_continues_while_a_members_name_is_typed() {
        let source = r#"
PROGRAM Main
VAR
    Timer : TON;
END_VAR
    Timer.E|
END_PROGRAM
"#;
        let labels = sorted_labels_at(source);
        assert!(labels.iter().any(|label| label == "ET"), "{labels:?}");
        assert!(
            labels
                .iter()
                .all(|label| ["ET", "IN", "PT", "Q"].contains(&label.as_str())),
            "only TON's members: {labels:?}"
        );
    }

    /// Specification 14 §6.1.2: after `.` completion does not offer a member with `{attribute 'hide'}`, and offers the
    /// members beside it; code may still name the hidden one. A function block offers its methods (§6.1.2's table),
    /// and under the CODESYS profile its plain `VAR` members too, which are public there (DEV-028). Catches a hidden
    /// member offered, and a visible member beside it dropped.
    #[test]
    fn member_completion_leaves_out_hidden_members() {
        let source = r#"
FUNCTION_BLOCK FB_Pump
VAR
    level : INT;
    {attribute 'hide'}
    secret : BOOL;
END_VAR

METHOD PUBLIC Start
END_METHOD

{attribute 'hide'}
METHOD PUBLIC Calibrate
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    pump : FB_Pump;
END_VAR
    pump.Calibrate();
    pump.|
END_PROGRAM
"#;
        let labels = labels_at(source);
        assert!(labels.iter().any(|label| label == "Start"), "{labels:?}");
        assert!(!labels.iter().any(|label| label == "Calibrate"), "{labels:?}");

        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);
        let mut db = Database::new();
        db.set_dialect(trust_hir::Dialect::Codesys);
        db.set_source_text(FileId(0), cleaned);
        let labels: Vec<String> = complete(&db, FileId(0), TextSize::from(cursor as u32))
            .into_iter()
            .map(|item| item.label.to_string())
            .collect();
        for shown in ["level", "Start"] {
            assert!(labels.iter().any(|label| label == shown), "{shown} missing: {labels:?}");
        }
        for hidden in ["secret", "Calibrate"] {
            assert!(!labels.iter().any(|label| label == hidden), "{hidden} offered: {labels:?}");
        }
    }
}
