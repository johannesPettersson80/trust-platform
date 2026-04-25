use super::super::queries::*;
use super::super::*;
use super::context::is_pou_kind;

const COMPLEXITY_WARN_THRESHOLD: usize = 15;
const MAX_RELATED_POINTS: usize = 3;

pub(in crate::db) fn check_cyclomatic_complexity(
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for pou in root.descendants().filter(|node| {
        matches!(
            node.kind(),
            SyntaxKind::Program
                | SyntaxKind::Function
                | SyntaxKind::FunctionBlock
                | SyntaxKind::Method
                | SyntaxKind::Property
        )
    }) {
        let Some((name, range)) = name_from_node(&pou) else {
            continue;
        };
        let (complexity, decision_points) = cyclomatic_complexity(&pou);
        if complexity <= COMPLEXITY_WARN_THRESHOLD {
            continue;
        }
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::HighComplexity,
            range,
            format!(
                "Cyclomatic complexity {} exceeds {} in '{}'",
                complexity, COMPLEXITY_WARN_THRESHOLD, name
            ),
        );
        for range in decision_points.into_iter().take(MAX_RELATED_POINTS) {
            diagnostic = diagnostic.with_related(range, "Decision point");
        }
        diagnostics.add(diagnostic);
    }
}

fn cyclomatic_complexity(pou: &SyntaxNode) -> (usize, Vec<TextRange>) {
    let mut decision_points = Vec::new();
    let mut stack = pou.children().collect::<Vec<_>>();
    stack.reverse();
    while let Some(node) = stack.pop() {
        if is_pou_kind(node.kind()) {
            continue;
        }
        match node.kind() {
            SyntaxKind::IfStmt
            | SyntaxKind::ElsifBranch
            | SyntaxKind::ForStmt
            | SyntaxKind::WhileStmt
            | SyntaxKind::RepeatStmt
            | SyntaxKind::CaseBranch => {
                decision_points.push(node.text_range());
            }
            _ => {}
        }
        let mut children = node.children().collect::<Vec<_>>();
        children.reverse();
        stack.extend(children);
    }
    (1 + decision_points.len(), decision_points)
}
