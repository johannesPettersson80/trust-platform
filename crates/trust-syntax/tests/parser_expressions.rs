mod common;
use common::*;
use trust_syntax::parser::parse;
use trust_syntax::syntax::SyntaxNode;

fn first_sizeof_expr(source: &str) -> SyntaxNode {
    let parsed = parse(source);
    assert!(parsed.ok(), "parse errors: {:?}", parsed.errors());
    parsed
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::SizeOfExpr)
        .expect("missing SizeOfExpr")
}

fn sizeof_children_kinds(source: &str) -> Vec<SyntaxKind> {
    first_sizeof_expr(source)
        .children()
        .map(|child| child.kind())
        .collect()
}

fn compact_text(node: &SyntaxNode) -> String {
    node.text()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn binary_expr_with_compact_text(source: &str, expected: &str) -> SyntaxNode {
    let parsed = parse(source);
    assert!(parsed.ok(), "parse errors: {:?}", parsed.errors());
    parsed
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::BinaryExpr && compact_text(node) == expected)
        .unwrap_or_else(|| panic!("missing BinaryExpr with compact text {expected}"))
}

// Expressions
#[test]
// IEC 61131-3 Ed.3 Table 71 (arithmetic operators)
fn test_arithmetic_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 1 + 2 - 3 * 4 / 5 MOD 6;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (comparison operators)
fn test_comparison_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    b := (x = 1) OR (x <> 2) OR (x < 3) OR (x <= 4) OR (x > 5) OR (x >= 6);
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (logical operators)
fn test_logical_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    b := a AND b OR c XOR d;
    c := NOT a;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (operator precedence)
fn test_operator_precedence() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 1 + 2 * 3;
    y := (1 + 2) * 3;
    z := 2 ** 3 ** 2;
END_PROGRAM"#
    ));
}

#[test]
fn exponentiation_is_left_associative_per_iec_table_71() {
    let expr = binary_expr_with_compact_text(
        r#"PROGRAM Test
    z := 2 ** 3 ** 2;
END_PROGRAM"#,
        "2**3**2",
    );
    let children = expr.children().collect::<Vec<_>>();

    assert_eq!(
        children.first().map(SyntaxNode::kind),
        Some(SyntaxKind::BinaryExpr),
        "outer power expression should group the left operand first: {expr:#?}"
    );
    assert_eq!(compact_text(&children[0]), "2**3");
}

#[test]
fn unary_minus_binds_tighter_than_exponentiation_per_iec_table_71() {
    let parsed = parse(
        r#"PROGRAM Test
    z := -2 ** 2;
END_PROGRAM"#,
    );
    assert!(parsed.ok(), "parse errors: {:?}", parsed.errors());

    let expr = parsed
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::BinaryExpr && compact_text(node) == "-2**2")
        .expect("outer expression should be exponentiation with unary left operand");
    let children = expr.children().collect::<Vec<_>>();

    assert!(
        children
            .iter()
            .any(|child| child.kind() == SyntaxKind::UnaryExpr && compact_text(child) == "-2"),
        "IEC Table 71 gives unary minus higher precedence than exponentiation: {expr:#?}"
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (unary operators)
fn test_unary_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := -a;
    y := +b;
    z := NOT c;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (function call syntax)
fn test_function_call() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := MyFunc(1, 2, 3);
    y := Add(a := 1, b := 2);
END_PROGRAM"#
    ));
}

#[test]
fn unclosed_call_argument_list_reports_missing_rparen() {
    let parsed = parse(
        r#"PROGRAM Test
    x := MyFunc(1, 2;
END_PROGRAM"#,
    );

    assert!(!parsed.ok(), "missing call ')' should be a parse error");
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message == "expected )"),
        "expected missing ')' diagnostic, got {:?}",
        parsed.errors()
    );
}

#[test]
fn test_time_builtin_call() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    t := TIME();
END_PROGRAM"#
    ));
}

#[test]
fn test_field_access() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := obj.field;
    y := obj.nested.deep;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (array indexing)
fn test_array_indexing() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := arr[0];
    y := matrix[i, j];
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (dereference operator)
fn test_pointer_dereference() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := ptr^;
    y := ptr^^;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (ADR/SIZEOF standard operators)
fn test_adr_sizeof() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    ptr := ADR(x);
    size := SIZEOF(INT);
END_PROGRAM"#
    ));
}

#[test]
fn test_sizeof_variable_operand_is_expression_not_type_ref() {
    let kinds = sizeof_children_kinds(
        r#"PROGRAM Test
    size := SIZEOF(x);
END_PROGRAM"#,
    );
    assert!(kinds.contains(&SyntaxKind::NameRef), "kinds: {kinds:?}");
    assert!(!kinds.contains(&SyntaxKind::TypeRef), "kinds: {kinds:?}");
}

#[test]
fn test_sizeof_field_operand_is_expression_not_type_ref() {
    let kinds = sizeof_children_kinds(
        r#"PROGRAM Test
    size := SIZEOF(box.value);
END_PROGRAM"#,
    );
    assert!(kinds.contains(&SyntaxKind::FieldExpr), "kinds: {kinds:?}");
    assert!(!kinds.contains(&SyntaxKind::TypeRef), "kinds: {kinds:?}");
}

#[test]
fn test_sizeof_index_operand_is_expression_not_type_ref() {
    let kinds = sizeof_children_kinds(
        r#"PROGRAM Test
    size := SIZEOF(arr[i]);
END_PROGRAM"#,
    );
    assert!(kinds.contains(&SyntaxKind::IndexExpr), "kinds: {kinds:?}");
    assert!(!kinds.contains(&SyntaxKind::TypeRef), "kinds: {kinds:?}");
}

#[test]
fn test_sizeof_deref_operand_is_expression_not_type_ref() {
    let kinds = sizeof_children_kinds(
        r#"PROGRAM Test
    size := SIZEOF(ptr^);
END_PROGRAM"#,
    );
    assert!(kinds.contains(&SyntaxKind::DerefExpr), "kinds: {kinds:?}");
    assert!(!kinds.contains(&SyntaxKind::TypeRef), "kinds: {kinds:?}");
}

#[test]
fn test_sizeof_call_operand_is_expression_not_type_ref() {
    let kinds = sizeof_children_kinds(
        r#"FUNCTION Value : DINT
Value := DINT#1;
END_FUNCTION
PROGRAM Test
    size := SIZEOF(Value());
END_PROGRAM"#,
    );
    assert!(kinds.contains(&SyntaxKind::CallExpr), "kinds: {kinds:?}");
    assert!(!kinds.contains(&SyntaxKind::TypeRef), "kinds: {kinds:?}");
}

#[test]
fn test_sizeof_explicit_builtin_type_operand_is_type_ref() {
    let kinds = sizeof_children_kinds(
        r#"PROGRAM Test
    size := SIZEOF(DINT);
END_PROGRAM"#,
    );
    assert!(kinds.contains(&SyntaxKind::TypeRef), "kinds: {kinds:?}");
}

#[test]
fn test_sizeof_explicit_array_type_operand_is_type_ref() {
    let kinds = sizeof_children_kinds(
        r#"PROGRAM Test
    size := SIZEOF(ARRAY[0..3] OF BYTE);
END_PROGRAM"#,
    );
    assert!(kinds.contains(&SyntaxKind::TypeRef), "kinds: {kinds:?}");
}

#[test]
fn test_this_super() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION_BLOCK FB_Test
    METHOD DoWork
        THIS.value := 1;
        SUPER.DoWork();
    END_METHOD
END_FUNCTION_BLOCK"#
    ));
}

#[test]
fn test_siemens_hash_prefixed_locals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    #temp := #input + #fb.value;
    #fb(Enable := #temp > 0);
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 31 (`A:= AND(B, C, D)`) and Table 29 (`MOD`): the functions by their names.
fn bitwise_and_modulo_functions_are_called_by_their_names() {
    // In operand position AND, OR, XOR and MOD followed by `(` name their function: a call whose
    // callee is that keyword, with its arguments. After an operand they stay infix operators, and a
    // bare keyword in operand position stays an error. Catches `AND(B, C, D)` refused as "expected
    // expression", and an infix `a AND (b)` read as a call.
    for (call, name, arguments) in [
        ("AND(a, b, c)", "AND", 3),
        ("or(a, b)", "or", 2),
        ("XOR(a, b)", "XOR", 2),
        ("MOD(a, 3)", "MOD", 2),
    ] {
        let parsed = parse(&format!("PROGRAM Test\n    x := {call};\nEND_PROGRAM"));
        assert!(parsed.ok(), "{call}: {:?}", parsed.errors());
        let call_expr = parsed
            .syntax()
            .descendants()
            .find(|node| node.kind() == SyntaxKind::CallExpr)
            .unwrap_or_else(|| panic!("{call}: no call"));
        let callee = call_expr.first_child().expect("the callee");
        assert_eq!(callee.kind(), SyntaxKind::NameRef, "{call}");
        assert_eq!(callee.text().to_string().trim(), name, "{call}");
        let argument_count = call_expr
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Arg)
            .count();
        assert_eq!(argument_count, arguments, "{call}");
    }
    let infix = parse("PROGRAM Test\n    x := a AND (b);\nEND_PROGRAM");
    assert!(infix.ok(), "{:?}", infix.errors());
    assert!(
        infix
            .syntax()
            .descendants()
            .all(|node| node.kind() != SyntaxKind::CallExpr),
        "a AND (b) is no call"
    );
    let bare = parse("PROGRAM Test\n    x := AND;\nEND_PROGRAM");
    assert!(!bare.ok(), "a bare AND is no operand");
}
