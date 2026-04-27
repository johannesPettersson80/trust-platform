use super::super::*;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::db) struct ExpressionIndex {
    by_range: FxHashMap<(u32, u32), u32>,
    ranges: Vec<(TextRange, u32)>,
}

impl ExpressionIndex {
    pub(in crate::db) fn from_root(root: &SyntaxNode) -> Self {
        let mut by_range = FxHashMap::default();
        let mut ranges = Vec::new();
        for (index, node) in root
            .descendants()
            .filter(|node| is_expression_kind(node.kind()))
            .enumerate()
        {
            let Ok(expr_id) = u32::try_from(index) else {
                break;
            };
            let range = node.text_range();
            by_range.insert(range_key(range), expr_id);
            ranges.push((range, expr_id));
        }
        Self { by_range, ranges }
    }

    pub(in crate::db) fn id_for_range(&self, range: TextRange) -> Option<u32> {
        self.by_range.get(&range_key(range)).copied()
    }

    pub(in crate::db) fn range_key_for_id(&self, expr_id: u32) -> Option<(u32, u32)> {
        self.ranges
            .get(expr_id as usize)
            .and_then(|(range, id)| (*id == expr_id).then_some(range_key(*range)))
    }

    pub(in crate::db) fn id_at_offset(&self, offset: TextSize) -> Option<u32> {
        self.ranges
            .iter()
            .filter(|(range, _)| range.contains(offset))
            .min_by_key(|(range, _)| range.len())
            .map(|(_, expr_id)| *expr_id)
    }
}

fn range_key(range: TextRange) -> (u32, u32) {
    (u32::from(range.start()), u32::from(range.end()))
}

pub(in crate::db) fn expression_by_id(root: &SyntaxNode, expr_id: u32) -> Option<SyntaxNode> {
    for (index, node) in root
        .descendants()
        .filter(|node| is_expression_kind(node.kind()))
        .enumerate()
    {
        let Ok(index) = u32::try_from(index) else {
            break;
        };
        if index == expr_id {
            return Some(node);
        }
    }
    None
}

pub(in crate::db) fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_initializer_expression_node()
}
