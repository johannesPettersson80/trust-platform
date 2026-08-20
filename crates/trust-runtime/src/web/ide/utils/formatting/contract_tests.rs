use super::*;

#[test]
fn empty_document_stays_empty() {
    assert_eq!(format_structured_text_document(""), "");
}

#[test]
fn nonempty_document_receives_exactly_one_terminal_newline() {
    assert_eq!(
        format_structured_text_document("PROGRAM Main\nEND_PROGRAM"),
        "PROGRAM Main\nEND_PROGRAM\n"
    );
    assert_eq!(
        format_structured_text_document("PROGRAM Main\nEND_PROGRAM\n"),
        "PROGRAM Main\nEND_PROGRAM\n"
    );
}

#[test]
fn formatter_uses_two_spaces_per_nested_level() {
    let source =
        "PROGRAM Main\nVAR\nx : INT;\nEND_VAR\nIF x > 0 THEN\nx := x - 1;\nEND_IF\nEND_PROGRAM";
    assert_eq!(
        format_structured_text_document(source),
        "PROGRAM Main\n  VAR\n    x : INT;\n  END_VAR\n  IF x > 0 THEN\n    x := x - 1;\n  END_IF\nEND_PROGRAM\n"
    );
}

#[test]
fn branch_continuations_dedent_then_indent_their_body() {
    let source =
        "IF ready THEN\nrun := TRUE;\nELSIF waiting THEN\nhold := TRUE;\nELSE\nstop := TRUE;\nEND_IF";
    assert_eq!(
        format_structured_text_document(source),
        "IF ready THEN\n  run := TRUE;\nELSIF waiting THEN\n  hold := TRUE;\nELSE\n  stop := TRUE;\nEND_IF\n"
    );
}

#[test]
fn repeat_until_dedents_until_line() {
    assert_eq!(
        format_structured_text_document("REPEAT\nx := x + 1;\nUNTIL x = 10"),
        "REPEAT\n  x := x + 1;\nUNTIL x = 10\n"
    );
}

#[test]
fn trailing_horizontal_whitespace_is_removed() {
    assert_eq!(
        format_structured_text_document("PROGRAM Main  \t\n  END_PROGRAM\t"),
        "PROGRAM Main\nEND_PROGRAM\n"
    );
}

#[test]
fn blank_lines_are_preserved() {
    assert_eq!(
        format_structured_text_document("PROGRAM Main\n\n\nEND_PROGRAM"),
        "PROGRAM Main\n\n\nEND_PROGRAM\n"
    );
}

#[test]
fn comment_text_is_preserved_at_current_indent() {
    assert_eq!(
        format_structured_text_document(
            "PROGRAM Main\n// keep  internal spacing\n(* block text *)\nEND_PROGRAM"
        ),
        "PROGRAM Main\n  // keep  internal spacing\n  (* block text *)\nEND_PROGRAM\n"
    );
}

#[test]
fn formatting_is_idempotent() {
    let once = format_structured_text_document(
        "FUNCTION_BLOCK Motor\nVAR_INPUT\nstart : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK",
    );
    assert_eq!(format_structured_text_document(&once), once);
}

#[test]
fn pou_and_object_openers_indent() {
    for line in [
        "PROGRAM Main",
        "FUNCTION F : INT",
        "FUNCTION_BLOCK Fb",
        "CONFIGURATION C",
        "RESOURCE R ON PLC",
        "CLASS C",
        "INTERFACE I",
        "METHOD M",
        "PROPERTY P : INT",
        "ACTION A",
        "TRANSITION T",
    ] {
        assert!(is_indent_line(line), "{line}");
    }
}

#[test]
fn all_variable_section_openers_indent() {
    for line in [
        "VAR",
        "VAR RETAIN",
        "VAR_INPUT",
        "VAR_OUTPUT",
        "VAR_IN_OUT",
        "VAR_TEMP",
        "VAR_GLOBAL",
        "VAR_EXTERNAL",
        "VAR_CONFIG",
        "VAR_ACCESS",
    ] {
        assert!(is_indent_line(line), "{line}");
    }
}

#[test]
fn conditional_and_loop_openers_require_their_complete_shape() {
    for line in [
        "IF x THEN",
        "CASE x OF",
        "FOR i := 0 TO 3 DO",
        "WHILE ready DO",
        "REPEAT",
    ] {
        assert!(is_indent_line(line), "{line}");
    }
    for line in ["IF x", "CASE x", "FOR i := 0 TO 3", "WHILE ready"] {
        assert!(!is_indent_line(line), "{line}");
    }
}

#[test]
fn end_and_branch_lines_dedent() {
    for line in [
        "END_PROGRAM",
        "END_VAR",
        "END_IF",
        "END_CASE",
        "ELSE",
        "ELSE // branch",
        "ELSIF x THEN",
        "UNTIL done",
    ] {
        assert!(is_dedent_line(line), "{line}");
    }
}

#[test]
fn ordinary_statements_neither_indent_nor_dedent() {
    for line in ["x := 1;", "RETURN;", "motor();", "// comment"] {
        assert!(!is_indent_line(line), "{line}");
        assert!(!is_dedent_line(line), "{line}");
    }
}
