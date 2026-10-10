use super::literals::exact_int_literal_value;
use super::*;

include!("stmt_impl_part_01.rs");
include!("stmt_impl_part_02.rs");
include!("stmt_impl_part_03.rs");
include!("stmt_impl_part_04.rs");
include!("stmt_impl_part_05.rs");

fn assignment_is_ref(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|token| token.kind() == SyntaxKind::RefAssign)
}

fn assignment_is_ref_bind(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|token| token.kind() == SyntaxKind::KwRefBind)
}
