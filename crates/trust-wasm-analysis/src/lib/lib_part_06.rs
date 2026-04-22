use rowan::{NodeOrToken, WalkEvent};
use trust_syntax::parser::parse;
use trust_syntax::syntax::SyntaxKind;

const CANONICAL_AST_ALGORITHM: &str = "canonical_ast_jaccard_5gram_v1";
const CANONICAL_AST_GRAM_SIZE: usize = 5;

pub fn canonical_ast_summary(source: &str) -> CanonicalAstSummary {
    let parsed = parse(source);
    let stream = canonical_ast_stream(&parsed.syntax());
    let five_grams = canonical_five_grams(&stream);
    CanonicalAstSummary {
        algorithm: CANONICAL_AST_ALGORITHM.to_string(),
        gram_size: CANONICAL_AST_GRAM_SIZE,
        parse_error_count: parsed.errors().len(),
        stream,
        five_grams,
    }
}

pub fn canonical_ast_similarity(left: &str, right: &str) -> AstSimilarityResult {
    let left_summary = canonical_ast_summary(left);
    let right_summary = canonical_ast_summary(right);

    let left_set: BTreeSet<&str> = left_summary.five_grams.iter().map(String::as_str).collect();
    let right_set: BTreeSet<&str> = right_summary.five_grams.iter().map(String::as_str).collect();
    let shared_grams = left_set.intersection(&right_set).count();
    let union_grams = left_set.len() + right_set.len() - shared_grams;
    let score = if union_grams == 0 {
        1.0
    } else {
        shared_grams as f32 / union_grams as f32
    };

    AstSimilarityResult {
        algorithm: CANONICAL_AST_ALGORITHM.to_string(),
        gram_size: CANONICAL_AST_GRAM_SIZE,
        score,
        shared_grams,
        left_grams: left_set.len(),
        right_grams: right_set.len(),
        threshold_070: score > 0.70,
        threshold_095: score > 0.95,
    }
}

fn canonical_ast_stream(root: &trust_syntax::SyntaxNode) -> Vec<String> {
    let mut stream = Vec::new();

    for event in root.preorder_with_tokens() {
        let WalkEvent::Enter(element) = event else {
            continue;
        };
        match element {
            NodeOrToken::Node(node) => stream.push(format!("N:{:?}", node.kind())),
            NodeOrToken::Token(token) => {
                let kind = token.kind();
                if kind.is_trivia() || kind == SyntaxKind::Eof {
                    continue;
                }
                stream.push(format!("T:{kind:?}"));
            }
        }
    }

    stream
}

fn canonical_five_grams(stream: &[String]) -> Vec<String> {
    if stream.len() < CANONICAL_AST_GRAM_SIZE {
        return Vec::new();
    }

    let mut grams = BTreeSet::new();
    for window in stream.windows(CANONICAL_AST_GRAM_SIZE) {
        grams.insert(window.join(" "));
    }
    grams.into_iter().collect()
}

#[cfg(test)]
mod canonical_ast_tests {
    use super::{canonical_ast_similarity, canonical_ast_summary, CANONICAL_AST_ALGORITHM};

    #[test]
    fn canonical_ast_strips_comments_and_identifier_values() {
        let left = "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n(* comment *)\nCounter := 1;\nEND_PROGRAM\n";
        let right = "PROGRAM Demo\nVAR\nValue : INT;\nEND_VAR\nValue := 99;\nEND_PROGRAM\n";

        let left_summary = canonical_ast_summary(left);
        let right_summary = canonical_ast_summary(right);

        assert_eq!(left_summary.algorithm, CANONICAL_AST_ALGORITHM);
        assert_eq!(left_summary.stream, right_summary.stream);
        assert_eq!(left_summary.five_grams, right_summary.five_grams);
    }

    #[test]
    fn canonical_ast_similarity_clears_high_threshold_for_structural_equivalence() {
        let left = "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nCounter := 1;\nEND_PROGRAM\n";
        let right = "PROGRAM Demo\nVAR\nValue : INT;\nEND_VAR\nValue := 42;\nEND_PROGRAM\n";

        let similarity = canonical_ast_similarity(left, right);
        assert!(similarity.threshold_095, "{similarity:?}");
        assert!(similarity.threshold_070, "{similarity:?}");
        assert_eq!(similarity.score, 1.0);
    }

    #[test]
    fn canonical_ast_similarity_drops_below_contamination_threshold_for_structural_change() {
        let left = "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nCounter := Counter + 1;\nEND_PROGRAM\n";
        let right = "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nIF Counter > 0 THEN\nCounter := Counter + 1;\nEND_IF\nEND_PROGRAM\n";

        let similarity = canonical_ast_similarity(left, right);
        assert!(!similarity.threshold_070, "{similarity:?}");
        assert!(!similarity.threshold_095, "{similarity:?}");
        assert!(similarity.score < 0.70, "{similarity:?}");
    }
}
